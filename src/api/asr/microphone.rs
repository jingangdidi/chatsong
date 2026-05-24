use std::sync::{Arc, Mutex};
use std::thread;

use cpal::{
    traits::{
        DeviceTrait,
        HostTrait,
        StreamTrait,
    },
    SampleFormat,
};
use tokio::sync::mpsc::{
    Sender,
    Receiver,
};
use tokio::sync::oneshot;
use tracing::{event, Level};

use crate::{
    //parse_paras::PARAS,
    error::MyError,
};

/// capture audio from microphone
pub async fn start_audio_capture(tx: Sender<Vec<f32>>, mut rx_start: Receiver<(bool, oneshot::Sender<()>)>) -> Result<(u32, u16), MyError> {
    let start_capture = Arc::new(Mutex::new(true));
    let start_capture_clone = Arc::clone(&start_capture);

    tokio::spawn(async move {
        while let Some((new_start, ack_tx)) = rx_start.recv().await {
            let mut guard = start_capture_clone.lock().unwrap();
            *guard = new_start;
            ack_tx.send(()).unwrap(); // 发送确认
        }
    });

    // 1. initialise the Host
    let host = cpal::default_host();
    // https://docs.rs/cpal/0.16.0/cpal/platform/struct.Host.html#method.default_input_device
    let device = match host.default_input_device() {
        Some(d) => d,
        None => return Err(MyError::OtherError{info: "No default input device available".to_string()})
    };
    event!(Level::INFO, "Using default input device: {}", device.name().map_err(|e| MyError::RetrieveDeviceNameError{error: e})?);

    // 2. get stream config
    /* linux没问题，但macos会报错
    let config = match device
        .supported_input_configs().map_err(|e| MyError::RetrieveSupportedStreamConfigError{error: e})?
        .find(|range| {
            range.channels() == 1 // mono
                && range.sample_format() == SampleFormat::F32
                && 24000 >= range.min_sample_rate().0
                && 24000 <= range.max_sample_rate().0
        }) {
            Some(supported_config_range) => {
                match supported_config_range.try_with_sample_rate(cpal::SampleRate(24000)) {
                    Some(config) => config,
                    None => device.default_input_config().map_err(|e| MyError::GetDefaultStreamConfigError{error: e})?,
                }
            },
            None => device.default_input_config().map_err(|e| MyError::GetDefaultStreamConfigError{error: e})?,
    };
    */
    let config = device.default_input_config().map_err(|e| MyError::OtherError{info: format!("{:?}", e)})?;

    // 2. The default input stream format for the device.
    // https://docs.rs/cpal/0.16.0/cpal/platform/struct.Device.html#method.default_input_config
    //let config = device.default_input_config().map_err(|e| MyError::GetDefaultStreamConfigError{error: e})?;
    let sample_rate: u32 = config.sample_rate().0;
    event!(Level::INFO, "capture audio sample rate: {}", sample_rate);
    let channels: u16 = config.channels();
    event!(Level::INFO, "capture audio sample channels: {}", channels);

    // 3. create stream
    // https://docs.rs/cpal/0.16.0/cpal/struct.SupportedStreamConfig.html#method.sample_format
    // https://docs.rs/cpal/0.16.0/cpal/platform/struct.Device.html#method.build_input_stream
    // 在独立线程中创建并持有 Stream（Stream 不是 Send，不能在 async 间传递）
    let _thread = thread::spawn(move || {
        let stream_result = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let start_capture = {
                        let guard = start_capture.lock().unwrap();
                        *guard
                    };
                    if start_capture {
                        //println!("32");
                        // 将数据复制到新的 Vec 中并发送
                        // try_send 用于非异步上下文，如果通道满了，它会返回错误
                        // 在这里我们简单地丢弃错误（即丢弃一帧音频），避免阻塞音频线程
                        let _ = tx.try_send(data.to_vec());
                    }
                },
                move |err| event!(Level::ERROR, "audio stream f32: {}", err),
                None,
            ).map_err(|e| MyError::BuildInputStreamError{error: e}),
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let start_capture = {
                        let guard = start_capture.lock().unwrap();
                        *guard
                    };
                    if start_capture {
                        //println!("16");
                        // 将 i16 数据归一化到 f32 [-1.0, 1.0] 范围
                        let f32_data: Vec<f32> = data
                            .iter()
                            .map(|&sample| f32::from(sample) / i16::MAX as f32)
                            .collect();
                        let _ = tx.try_send(f32_data);
                    }
                },
                move |err| event!(Level::ERROR, "audio stream i16: {}", err),
                None,
            ).map_err(|e| MyError::BuildInputStreamError{error: e}),
            sample_format => Err(MyError::OtherError{info: format!("Unsupported sample format {}", sample_format)}),
        };

        match stream_result {
            Ok(stream) => {
                // 4. start stream
                if let Err(e) = stream.play() {
                    event!(Level::ERROR, "{}", e);
                } else {
                    event!(Level::INFO, "Audio capture started");
                }

                // 永久阻塞，保持 Stream 存活
                // loop + park 可处理虚假唤醒，线程在此处休眠直到被彻底释放（但永远不会）
                loop {
                    thread::park();
                }
            },
            Err(e) => event!(Level::ERROR, "{}", e),
        }
    });

    Ok((sample_rate, channels))
}
