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
        If you are not satisfied with the answer obtained from the input question and want to ask again using a different model, you do not need to input any content. Simply press enter and the last question will be asked again without the need to input the same question again.
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
    It is divided into two parts: \"front\" and \"back\". \"Front\" is a commonly used parameter, while \"back\" is an infrequently used parameter. By default, only the \"front\" parameter is displayed. You can click the settings button in the lower left corner to switch between the \"front\" and \"back\".
    Front parameters:
        1. start new chat
            Select a prompt to initiate a dialogue; otherwise, continue the current conversation.
        2. new chat title (optional)
            Assign a name for easy navigation between dialogues.
        3. models
            Choose the active model (switchable mid-dialogue).
        4. contextual messages
            Every time you submit a question, you need to bring along the previous few conversation messages (including the question and reply, including the current submitted question), discard earlier irrelevant conversation messages, and save tokens as the smaller the message.
            A pair of Q&A includes: one or more consecutive question messages and one or more consecutive answer messages.
            unlimit: Every time a question is submitted, all previous Q&A records will be included.
            1 Q&A pair: Only include the current input question (which can be multiple consecutive messages); If there are no input questions, it is the last question (which can be multiple consecutive messages).
            2 Q&A pairs: Contains all messages from the latest 1 Q&A pair, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is all the messages from the second to last Q&A pair, as well as the last question (which can be multiple consecutive messages).
            3 Q&A pairs: Contains all messages from the latest 2 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is all the messages from the third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            4 Q&A pairs: Contains all messages from the latest 3 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is all the messages from the fourth-to-last, third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            5 Q&A pairs: Contains all messages from the latest 3 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is all the messages from the fifth-to-last, fourth-to-last, third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            prompt + 1 Q&A pair: Only include prompt(if specified when creating the chat) and the current input question (which can be multiple consecutive messages); If there are no input questions, it is prompt(if specified when creating the chat) and the last question (which can be multiple consecutive messages).
            prompt + 2 Q&A pairs: Contains prompt(if specified when creating the chat) and all messages from the latest 1 Q&A pair, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is prompt(if specified when creating the chat) and all the messages from the second to last Q&A pair, as well as the last question (which can be multiple consecutive messages).
            prompt + 3 Q&A pairs: Contains prompt(if specified when creating the chat) and all messages from the latest 2 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is prompt(if specified when creating the chat) and all the messages from the third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            prompt + 4 Q&A pairs: Contains prompt(if specified when creating the chat) and all messages from the latest 3 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is prompt(if specified when creating the chat) and all the messages from the fourth-to-last, third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            prompt + 5 Q&A pairs: Contains prompt(if specified when creating the chat) and all messages from the latest 3 Q&A pairs, as well as the current input question (which can be multiple consecutive messages); If there is no input question, it is prompt(if specified when creating the chat) and all the messages from the fifth-to-last, fourth-to-last, third-to-last and second-to-last Q&A pairs, as well as the last question (which can be multiple consecutive messages).
            1 message: Only contains the current one input message; If there is no input question, it is the last question message.
            2 messages: Contains the latest 1 message and the current one input message; If there is no input question, the latest or consecutive answer message will be ignored, and 2 messages will be counted forward from the most recent question messages.
            3 messages: Contains the latest 2 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and 3 messages will be counted forward from the most recent question message.
            4 messages: Contains the latest 3 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and 4 messages will be counted forward from the most recent question message.
            5 messages: Contains the latest 4 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and 5 messages will be counted forward from the most recent question message.
            prompt + 1 message: Only contains prompt(if specified when creating the chat) and the current one input message; If there is no input question, it is prompt(if specified when creating the chat) and the last question message.
            prompt + 2 messages: Contains prompt(if specified when creating the chat) and the latest 1 message and the current one input message; If there is no input question, the latest or consecutive answer message will be ignored, and prompt(if specified when creating the chat) and 2 messages will be counted forward from the most recent question messages.
            prompt + 3 messages: Contains prompt(if specified when creating the chat) and the latest 2 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and prompt(if specified when creating the chat) and 3 messages will be counted forward from the most recent question message.
            prompt + 4 messages: Contains prompt(if specified when creating the chat) and the latest 3 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and prompt(if specified when creating the chat) and 4 messages will be counted forward from the most recent question message.
            prompt + 5 messages: Contains prompt(if specified when creating the chat) and the latest 4 messages, as well as the current one input message; If there is no input question, the latest or consecutive answer information will be ignored, and prompt(if specified when creating the chat) and 5 messages will be counted forward from the most recent question message.
        5. web search
            Yes: Queries leverage online resources or specified URLs.
            No: Responding based on the model's intrinsic knowledge repository.
        6. current prompt
            Displays the active prompt name.
        7. current uuid
            Identifier for the ongoing dialogue.
        8. input token
            The total input tokens used in the current dialogue.
        9. output token
            The total output tokens used in the current dialogue.
    Back parameters:
        1. reasoning effort
            Adjust reasoning effort (for compatible models).
        2. uuid
            Unique dialogue identifier. Enter a prior UUID to resume a previous dialogue.
        3. related UUIDs
            Dropdown list of past dialogues for quick access (lower priority than manual UUID entry).
        4. temperature
            Modulates output randomness.
        5. stream
            Yes: Real-time, incremental display.
            No: Wait for complete response before display.
        6. voice
            Choose audio synthesis voice (for TTS models).
", PARAS.addr_str, PARAS.port, uri.path())
    } else {
        format!("main page: http://{}:{}{}

对话说明
    1. 常规对话
        输入问题，回车提交问题，可连续输入多次（比如一个复杂的问题分多次进行描述），不输入内容直接回车表示输入完毕，等待回复，此时输入框无效。
        如果输入问题获取到答案后对答案不满意，想要换个模型再问一次，此时不需要输入任何内容，直接回车，就会把最后一个问题再问一次而不需要再输入一次同样的问题。
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
    分为“正面”和“反面”两部分，“正面”是常用的参数，“反面”是不常用的参数，默认只显示“正面”参数，可点击左下角的设置按钮切换正面和反面。
    正面参数：
        1. 开启新对话
            下拉选择一个prompt，会开启一个新对话，不选择则始终基于当前对话进行问答
        2.新对话名称（可选）
            开启新对话时，可以输入一个名称，方便在不同对话间切换时快速找到目标对话
        3. 模型
            选择要使用的模型，同一对话可以切换使用不同模型
        4. 上下文消息数
            每次提交问题时，需要带上之前几条对话信息（包括问题与回复，包括当前提交的问题），舍弃更早的无关对话信息，越小越节省token
            一对问答包括：一个或多个连续的问题信息和一个或多个连续的答案信息
            不限制：每次提交问题都带上之前所有的问答记录
            1对Q&A：只包含当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是最后一个问题（可以是多条连续的信息）
            2对Q&A：包含最新的1对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是倒数第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            3对Q&A：包含最新的2对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是倒数第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            4对Q&A：包含最新的3对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是倒数第4、第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            5对Q&A：包含最新的4对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是倒数第5、第4、第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            prompt + 1对Q&A：只包含prompt（如果创建该对话时有指定）和当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是prompt（如果创建该对话时有指定）和最后一个问题（可以是多条连续的信息）
            prompt + 2对Q&A：包含prompt（如果创建该对话时有指定）和最新的1对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是prompt（如果创建该对话时有指定）和倒数第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            prompt + 3对Q&A：包含prompt（如果创建该对话时有指定）和最新的2对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是prompt（如果创建该对话时有指定）和倒数第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            prompt + 4对Q&A：包含prompt（如果创建该对话时有指定）和最新的3对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是prompt（如果创建该对话时有指定）和倒数第4、第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            prompt + 5对Q&A：包含prompt（如果创建该对话时有指定）和最新的4对问答的所有信息，以及当前输入问题（可以是多条连续的信息）；如果没有输入问题，则是prompt（如果创建该对话时有指定）和倒数第5、第4、第3、第2对问答的所有信息，以及最后一个问题（可以是多条连续的信息）
            1条信息：只包含当前输入的一条问题信息；如果没有输入问题，则是最后一条问题信息
            2条信息：包含最新的1条回答信息，以及当前输入的一条问题信息；如果没有输入问题，则是除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数2条信息
            3条信息：包含最新的2条信息，以及当前输入的一条问题信息；如果没有输入问题，则是除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数3条信息
            4条信息：包含最新的3条信息，以及当前输入的一条问题信息；如果没有输入问题，则是除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数4条信息
            5条信息：包含最新的3条信息，以及当前输入的一条问题信息；如果没有输入问题，则是除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数5条信息
            prompt + 1条信息：只包含prompt（如果创建该对话时有指定）和当前输入的一条问题信息；如果没有输入问题，则是prompt（如果创建该对话时有指定）和最后一条问题信息
            prompt + 2条信息：包含prompt（如果创建该对话时有指定）和最新的1条回答信息，以及当前输入的一条问题信息；如果没有输入问题，则是prompt（如果创建该对话时有指定）和除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数2条信息
            prompt + 3条信息：包含prompt（如果创建该对话时有指定）和最新的2条信息，以及当前输入的一条问题信息；如果没有输入问题，则是prompt（如果创建该对话时有指定）和除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数3条信息
            prompt + 4条信息：包含prompt（如果创建该对话时有指定）和最新的3条信息，以及当前输入的一条问题信息；如果没有输入问题，则是prompt（如果创建该对话时有指定）和除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数4条信息
            prompt + 5条信息：包含prompt（如果创建该对话时有指定）和最新的3条信息，以及当前输入的一条问题信息；如果没有输入问题，则是prompt（如果创建该对话时有指定）和除去最新一条或连续的多条答案信息，从最近一次的问题信息往前数5条信息
        5. 网络搜索
            Yes表示对输入的问题进行网络搜索，或解析输入的url内容，基于搜索或解析的内容进行回答，No表示基于模型自身的知识库进行回答
        6. 当前prompt
            显示当前对话的prompt名称
        7. 当前uuid
            当前对话的uuid，使用该uuid可切换不同对话
        8. 输入的总token
            当前对话输入问题的token总数
        9. 输出的总token
            当前对话输出内容的token总数
    反面参数：
        1. 思考的深度
            设置思考的深度，仅对支持thinking的模型有效
        2. uuid
            每个对话会有一个uuid，可以输入之前对话的uuid切换到之前的对话，基于之前对话进行问答
        3. 相关uuid
            与当前uuid相关的uuid，可以下拉选择之前的对话，下次提问将跳转到选择的对话，相比输入uuid要方便，优先级没有输入uuid高
        4. 温度
            控制生成内容的随机性
        5. 流式输出
            Yes表示输出内容实时逐字显示，No表示等回答完成后一次性显示（在完成回答之前会一直等待）
        6. 声音
            生成音频时，可选择音频的声音
", PARAS.addr_str, PARAS.port, uri.path())
    })
}
