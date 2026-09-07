use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use xcap::Monitor;

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

/// A rectangle relative to the selected monitor's top-left corner.
#[derive(Clone, Copy, Deserialize)]
struct CaptureRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct Params {
    /// Destination PNG path. It must be inside an allowed directory.
    path: String,

    /// XCap monitor ID. When omitted, the primary monitor is used. 默认主屏
    #[serde(default)]
    monitor_id: Option<u32>,

    /// Optional region relative to the selected monitor.
    #[serde(default)]
    region: Option<CaptureRegion>,
}

/// Built-in tool for capturing a monitor or monitor region as a PNG file.
pub struct ScreenCapture;

impl ScreenCapture {
    /// new
    pub fn new() -> Self {
        ScreenCapture
    }
}

impl BuiltIn for ScreenCapture {
    /// get tool name
    fn name(&self) -> String {
        "screen_capture".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Captures the primary monitor, a selected monitor, or a region of that monitor and saves it as a PNG file inside an allowed directory. The returned attachment path can be supplied to a vision-capable model. Region coordinates are relative to the selected monitor's top-left corner.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Destination path for the screenshot. It must be inside an allowed directory and end with .png."
                },
                "monitor_id": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional xcap monitor ID. If omitted, the primary monitor is captured."
                },
                "region": {
                    "type": "object",
                    "description": "Optional capture rectangle relative to the selected monitor's top-left corner. Omit it to capture the whole monitor.",
                    "properties": {
                        "x": { "type": "integer", "minimum": 0 },
                        "y": { "type": "integer", "minimum": 0 },
                        "width": { "type": "integer", "minimum": 1 },
                        "height": { "type": "integer", "minimum": 1 }
                    },
                    "required": ["x", "y", "width", "height"],
                    "additionalProperties": false
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: Some(vec!["paths".to_string()]), object_fields: None })?;
        let result = self.capture(params)?;
        Ok((result, None))
    }

    /// get approval message
     fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        //let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        if is_en {
            Ok(Some(format!("Do you allow capturing screenshot?\n{}", info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 screen_capture 工具截屏？\n{}", info.unwrap_or_default())))
        }
    }
}

impl ScreenCapture {
    /// 选择要截取的屏幕
    fn select_monitor(monitors: Vec<Monitor>, monitor_id: Option<u32>) -> Result<Monitor, MyError> {
        if monitors.is_empty() {
            return Err(MyError::OtherError{info: "failed to select a monitor, xcap returned no monitors".to_string()})
        }

        if let Some(expected_id) = monitor_id {
            return monitors
                .into_iter()
                .find(|monitor| monitor.id().ok() == Some(expected_id))
                .ok_or_else(|| MyError::OtherError{info: format!("monitor_id {expected_id} was not found")});
        }

        let primary_index = monitors
            .iter()
            .position(|monitor| monitor.is_primary().unwrap_or(false))
            .unwrap_or(0);

        // Safe because the empty case was handled above and primary_index is in bounds.
        Ok(monitors.into_iter().nth(primary_index).unwrap())
    }

    /// 检查截屏区域
    fn validate_region(region: CaptureRegion, monitor_width: u32, monitor_height: u32) -> Result<(), MyError> {
        if region.width == 0 || region.height == 0 {
            return Err(MyError::OtherError{info: "region width and height must both be greater than zero".to_string()})
        }

        let right = region
            .x
            .checked_add(region.width)
            .ok_or_else(|| MyError::OtherError{info: "region x + width overflowed u32".to_string()})?;
        let bottom = region
            .y
            .checked_add(region.height)
            .ok_or_else(|| MyError::OtherError{info: "region y + height overflowed u32".to_string()})?;

        if right > monitor_width || bottom > monitor_height {
            return Err(MyError::OtherError{info: format!(
                "region ({}, {}, {}, {}) exceeds monitor bounds {}x{}",
                region.x,
                region.y,
                region.width,
                region.height,
                monitor_width,
                monitor_height
            )})
        }

        Ok(())
    }

    /// 截屏
    /// fn capture(&self, params: Params) -> Result<(Value, String), MyError> {
    fn capture(&self, params: Params) -> Result<String, MyError> {
        let output_path = Path::new(&params.path);
        let monitors = Monitor::all().map_err(|error| MyError::OtherError{info: format!("failed to enumerate monitors: {:?}", error)})?;
        let monitor = Self::select_monitor(monitors, params.monitor_id)?;

        let monitor_width = monitor
            .width()
            .map_err(|error| MyError::OtherError{info: format!("failed to read monitor width: {:?}", error)})?;
        let monitor_height = monitor
            .height()
            .map_err(|error| MyError::OtherError{info: format!("failed to read monitor height: {:?}", error)})?;
        let scale_factor = monitor.scale_factor().unwrap_or(1.0);

        let (image, _capture_kind, _capture_region) = match params.region {
            Some(region) => {
                Self::validate_region(region, monitor_width, monitor_height)?;
                let image = monitor
                    .capture_region(region.x, region.y, region.width, region.height)
                    .map_err(|error| MyError::OtherError{info: format!("failed to capture monitor region: {:?}", error)})?;
                (image, "region", Some(region))
            }
            None => {
                let image = monitor.capture_image().map_err(|error| MyError::OtherError{info: format!("failed to capture monitor region: {:?}", error)})?;
                (image, "monitor", None)
            }
        };

        image.save(output_path).map_err(|error| MyError::OtherError{info: format!("failed to save screenshot as PNG: {:?}", error)})?;
        /*
        let monitor_id = monitor.id().map_err(|error| MyError::OtherError{info: format!("failed to read monitor id: {:?}", error)})?;
        let monitor_name = monitor
            .friendly_name()
            .or_else(|_| monitor.name())
            .unwrap_or_else(|_| format!("monitor-{monitor_id}"));
        let monitor_x = monitor
            .x()
            .map_err(|error| MyError::OtherError{info: format!("failed to read monitor x coordinate", error)})?;
        let monitor_y = monitor
            .y()
            .map_err(|error| MyError::OtherError{info: format!("failed to read monitor y coordinate", error)})?;
        let is_primary = monitor.is_primary().unwrap_or(false);
        let image_width = image.width();
        let image_height = image.height();
        let output_path_string = output_path.to_string_lossy().into_owned();
        let result = json!({
            "success": true,
            "path": output_path_string,
            "format": "png",
            "capture": {
                "kind": capture_kind,
                "region": capture_region.map(|region| json!({
                    "x": region.x,
                    "y": region.y,
                    "width": region.width,
                    "height": region.height,
                })),
            },
            "image": {
                "width": image_width,
                "height": image_height,
            },
            "monitor": {
                "id": monitor_id,
                "name": monitor_name,
                "is_primary": is_primary,
                "x": monitor_x,
                "y": monitor_y,
                "width": monitor_width,
                "height": monitor_height,
                "scale_factor": scale_factor,
            }
        });
        Ok((result, output_path_string))
        */
        Ok(format!("Screenshot captured successfully (scale_factor={}): {}", scale_factor, params.path))
    }
}
