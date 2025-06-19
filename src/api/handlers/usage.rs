use axum::{http::StatusCode, extract::OriginalUri};
use tracing::{event, Level};

/// PARAS: 存储命令行参数的全局变量
use crate::parse_paras::PARAS;

/// Handler for `/嵌套的前缀/usage` GET
pub async fn usage(uri: OriginalUri) -> Result<String, StatusCode> {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    Ok(if PARAS.english {
        format!("main page: http://{}:{}{}

Usage Instructions
    1. Standard Dialogue
        Submit queries by pressing Enter after each input. Consecutive inputs are permitted. An empty submission indicates completion, rendering the input field inactive.
        For lengthy content, you can save the text to a file and upload it, then submit without additional input.
        Example: \"What is Rust programming language?\"

    2. Image Link Queries
        Format: \"img [URL]\", followed by the question. Submit with an empty line to finalize.
        Example: \"img http://example.com/image.jpg\"
                 \"Describe the contents of this image.\"

    3. Online Queries
        Enable \"web search\" in the sidebar. Submit the query, await results, then finalize with an empty submission.
        Alternatively, specify URLs followed by the question (space-separated).
        Examples: (1) \"Explain the Rust programming language.\"
                  (2) \"http://url1 http://url2 Explain the Rust programming language.\"
        Note: (1) Requires API keys in config.txt or command-line arguments (-e, -s).
              (2) URLs must precede the question, separated by spaces.

    4. File Upload Queries
        Upload files via the sidebar (supports HTML, PDF, ZIP, others as plain text).
        Submit the question afterward, finalized with an empty line.

    5. Image Generation
        For models gpt-image-1, dall-e-2, or dall-e-3: Input a description to generate art or upload an image for edits. Outputs are downloadable.

    6. MP3 Audio Generation
        For models tts-1 or tts-1-hd: Input text to synthesize speech. Audio files are downloadable.

    7. Audio Transcription/Translation
        For model whisper-1: Upload audio. Submit \"transc\" to extract text or \"transl\" for English translation.

Sidebar Settings
    1. start new chat
        Select a prompt to initiate a dialogue; otherwise, continue the current conversation.
    2. models
        Choose the active model (switchable mid-dialogue).
    3. reasoning effort
        Adjust reasoning effort (for compatible models).
    4. new chat title (optional)
        Assign a name for easy navigation between dialogues.
    5. uuid
        Unique dialogue identifier. Enter a prior UUID to resume a previous dialogue.
    6. related UUIDs
        Dropdown list of past dialogues for quick access (lower priority than manual UUID entry).
    7. temperature
        Modulates output randomness.
    8. stream
        Yes: Real-time, incremental display.
        No: Wait for complete response before display.
    9. web search
        Yes: Queries leverage online resources or specified URLs.
        No: Responding based on the model's intrinsic knowledge repository.
    10. send messages
        Configures how many prior messages (Q&A pairs) are included with submissions:
        unlimited: Retain all history or reset with current query.
        fixed counts: Include N prior messages (e.g., 5, 10, 100).  
        prompt-integrated options: Prepend the dialogue prompt if defined.
    11. voice
        Choose audio synthesis voice (for TTS models).
    12. Save chat log
        Download dialogue history as an HTML file (UUID and optional name in filename).
    13. file Upload
        Upload your files, multiple documents are supported.
    14. Usage
        Access documentation.
    15. current prompt
        Displays the active prompt name.
    16. current uuid
        Identifier for the ongoing dialogue.
    17. input token
        The total input tokens used in the current dialogue.
    18. output token
        The total output tokens used in the current dialogue.
", PARAS.addr_str, PARAS.port, uri.path())
    } else {
        format!("main page: http://{}:{}{}

对话说明
    1. 常规对话
        输入问题，回车提交问题，可连续输入多次，不输入内容直接回车表示输入完毕，等待回复，此时输入框无效。
        如果输入内容太长，也可将输入内容保存至文件中，上传文件，然后不输入内容直接回车即可。
        例如：“What is rust language?”

    2. 对图片链接进行提问
        输入“img httpxxx”，回车提交，再输入要问的问题，回车提交，不输入内容直接回车表示输入完毕，等待回复。
        例如：
            “img http:xxx”
            “what is in this picture?”

    3. 联网提问
        页面左侧点击开启“网络搜索”，输入问题，回车提交后等待搜索完成，然后不输入内容直接回车，即可基于搜索内容进行回答(示例1)。
        也可以指定空格间隔的1个或多个http链接，最后加上问题，从指定的url页面中提取内容进行回答(示例2)
        例如：
            (1) “What is rust language?”
            (2) “http://some-url-1 http://some-url-2 What is rust language?”
        注意：
            (1) 需要在命令行指定-e和-s，或config.txt中填写“google_engine_key”和“google_search_key”
            (2) 如果指定http链接时，需要把问题放在最后，url之间以及与问题之间需要用空格间隔

    4. 上传文件进行提问
        页面左侧点击上传文件，会自动解析内容，支持解析html文件、pdf文件、zip压缩文件，其他格式将被视为普通文本文件，上传文件后输入问题，回车提交问题，然后不输入内容直接回车即可。

    5. 绘图
        当选取的模型是gpt-image-1、dall-e-2、dall-e-3时，输入绘图要求进行绘图，或上传图片进行编辑。
        生成的图片会显示在页面，可点击下载。

    6. 生成mp3音频
        当选取的模型是tts-1、tts-1-hd时，输入内容生成音频。
        生成的音频会显示在页面，可点击下载。

    7. 从音频提取文本内容或翻译为英文
        当选取的模型是whisper-1时，上传音频文件。输入“transc”，从音频中提取文本；输入“transl”，将音频内容翻译为英文

页面左侧设置说明
    1. 开启新会话
        下拉选择一个prompt，会开启一个新对话，不选择则始终基于当前对话进行问答
    2. 模型
        选择要使用的模型，同一对话可以切换使用不同模型
    3. 思考的深度
        设置思考的深度，仅对支持thinking的模型有效
    4.新对话名称（可选）
        开启新对话时，可以输入一个名称，方便在不同对话间切换时快速找到目标对话
    5. uuid
        每个对话会有一个uuid，可以输入之前对话的uuid切换到之前的对话，基于之前对话进行问答
    6. 相关uuid
        与当前uuid相关的uuid，可以下拉选择之前的对话，下次提问将跳转到选择的对话，相比输入uuid要方便，优先级没有输入uuid高
    7. 温度
        控制生成内容的随机性
    8. 流式输出
        Yes表示输出内容实时逐字显示，No表示等回答完成后一次性显示（在完成回答之前会一直等待）
    9. 网络搜索
        Yes表示对输入的问题进行网络搜索，或解析输入的url内容，基于搜索或解析的内容进行回答，No表示基于模型自身的知识库进行回答
    10. 保留最新对话数
        每次提交问题时，需要带上之前几条对话记录（包括问题与回复，包括当前提交的问题），越小越节省token
        unlimit：无限制，每次提交问题都带上之前所有的问答记录
        unlimit+drop：舍弃本次提问前的所有记录，从本次提问开始无限制
        unlimit+prompt+drop：舍弃本次提问前的所有记录，如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上从本次提问开始的所有问答记录；如果该对话没有指定prompt，则与选择“unlimit+drop”相同
        1：每次提交问题只提交当前的问题
        1+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上当前的问题；如果该对话没有指定prompt，则与选择“1”相同
        1+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“1+prompt”的规则提问
        5：每次提交问题除了当前问题外，还包含之前的4条问答记录
        5+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上包括当前问题的最近5条记录；如果该对话没有指定prompt，则与选择“5”相同
        5+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“5+prompt”的规则提问
        10：每次提交问题除了当前问题外，还包含之前的9条问答记录
        10+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上包括当前问题的最近10条记录；如果该对话没有指定prompt，则与选择“10”相同
        10+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“10+prompt”的规则提问
        20：每次提交问题除了当前问题外，还包含之前的19条问答记录
        20+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上包括当前问题的最近20条记录；如果该对话没有指定prompt，则与选择“20”相同
        20+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“20+prompt”的规则提问
        50：每次提交问题除了当前问题外，还包含之前的49条问答记录
        50+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上包括当前问题的最近50条记录；如果该对话没有指定prompt，则与选择“50”相同
        50+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“50+prompt”的规则提问
        100：每次提交问题除了当前问题外，还包含之前的99条问答记录
        100+prompt：如果该对话有指定prompt，则始终将prompt作为提交问题的第一条信息，然后再加上包括当前问题的最近100条记录；如果该对话没有指定prompt，则与选择“100”相同
        100+prompt+drop：舍弃本次提问前的所有记录，从本次提问开始，按照“100+prompt”的规则提问
    11. 声音
        生成音频时，可选择音频的声音
    12. Save chat log
        点击可下载当前对话的问答记录，仅1个html文件，文件名含有该对话的uuid，如果创建该对话时输入了对话名称，也会包含在文件名中
    13. 上传文件
        点击选择要上传的文件，支持多选
    14. Usage
        查看使用说明
    15. current prompt
        显示当前对话的prompt名称
    16. current uuid
        当前对话的uuid，使用该uuid可切换不同对话
    17. input token
        当前对话输入问题的token总数
    18. output token
        当前对话输出内容的token总数
", PARAS.addr_str, PARAS.port, uri.path())
    })
}
