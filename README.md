# chatsong
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/jingangdidi/chatsong/blob/main/LICENSE)

[中文文档](https://github.com/jingangdidi/chatsong/blob/main/README_zh.md)

**A lightweight(~10M), portable executable for invoking LLM with multi-API support - eliminating installation requirements while maintaining operational efficiency.**

**轻量级大语言模型OpenAI格式api调用工具，无需安装，仅一个~10M可执行文件，支持自定义多种模型（OpenAI、Claude、Gemini、DeepSeek等，以及第三方提供的api）和prompt。**

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/demo_2x.gif">

## 👑 Features
- ​💪​ Single-file executable - no installation required
- 🔐 Privacy first, all data is stored locally
- 🤖 Unified multi-API support for LLM providers
- 🎨​ Customize models and prompts within the config file
- 1️⃣​ Support saving Q&A records as a single HTML file
- 📚​ Support invoking different models within the same conversation
- ​🌐​ Support web search and urls
- ​📤​ Support upload and parse zip, html, pdf, and text file
- 💻​ Support add local model in config.txt (e.g. provide by llama-server)
- ✨ Support markdown and code highlight
- 📊 Support counting the token usage for each conversation and message
- 💰 Support setting how many contextual messages to include in each query, greatly saving token usage
- ✂️ Support delete message
- 😎 Support incognito mode
- 📡 Support calling APIs compatible with the OpenAI format, such as Deepseek, Qwen, Z.AI GLM, and Moonshot Kimi
- 🔧 Support built-in filesystem tools
- 🔨 Support custom external tools and MCP stdio tools
- 🤔 Support planning mode

## 🚀 Quick-Start
**structure**
```
some dir
├─ chatsong   # single executable file
├─ config.txt # config file
└─ chat-log   # save chat log
```
**1. download a pre-built binary**

[latest release](https://github.com/jingangdidi/chatsong/releases)

**2. prepare config.txt**

add your models, api key, endpoint, etc, see [config_template.txt](https://github.com/jingangdidi/chatsong/blob/main/config_template.txt) for details.

**3. start server**

local usage:
```
./chatsong
```
If you want to start the service on computer A in the intranet and computer B accesses it, computer A needs to specify its own IP address when starting the service, which cannot be the default `127.0.0.1`.
It can be specified through the command-line parameter `-a <ip>`, for example, the IP of computer A is `192.168.1.5`:
```
./chatsong -a 192.168.1.5
```
You can also write it directly in the `config.txt`:
```
ip_address: "192.168.1.5",
```

**4. visit directly in your browser**

[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)

[http://192.168.1.5:8080/v1](http://192.168.1.5:8080/v1)

**5. terminate the service**
```
press `Ctrl+C` to automatically save all chat records to the output directory while simultaneously updating the graph file
```

## 🛠 Call tools
Starting from `v0.4.0`, chatsong supports calling tools. In addition to the built-in file system tools, you can also specify your own external tools and MCP's stdio tools through `SingleExternalTool` and `StdIoServer` in `config.txt`.

The `Call tools` dropdown option on the left side of the page supports the following options:

  - ⚪ white indicates not using any tools
  - 🔴 red indicates selecting all tools
  - 🟢 green indicates selecting built-in tools
  - 🟣 purple indicates selecting all custom external tools
  - 🟡 yellow indicates selecting MCP tools
  - other options indicate selecting only one tool

Add tools in `config.txt`:

**1. built-in tools**

  These tools have been compiled in `chatsong` and do not require additional configuration of `config.txt`. They can be called directly.

**2. custom external tools**

  `command`: fill in the command to be called

  `args`: fill in the script and other parameters

  `description`: fill in the functionality of the tool. The model will use this description to determine whether to use it to complete a task

  ```
  external_tools: [
    SingleExternalTool(
      name: "The name of Tool 1",
      command: "The command called by Tool 1, e.g., ./my_tool.exe",
      description: "The description of Tool 1",
      schema: r#"json format schema"#,
    ),
    SingleExternalTool(
      name: "The name of Tool 2",
      command: "The command called by Tool 1, e.g., python3",
      args: ["Script and other parameters are specified in this list, e.g., my_tool.py"],
      description: "The description of Tool 2",
      schema: r#"json format schema"#,
    )
  ]
  ```

  Note that the `schema` parameter type and description should be filled in JSON format. As it may contain `"` and line breaks, it should be placed between `r"#` and `"#`. For example, the following example is a Python script I wrote myself to calculate the sum of two numbers. The first parameter is `--a`, which specifies the first number, the second parameter is `--b`, which specifies the second number, the `type`, which specifies the parameter type, and the `description`, which describes the function of the parameter:
  ```
  schema: r#"
  {
    "properties": {
      "a": {
        "type": "integer",
        "description": "The first value.",
      },
      "b": {
        "type": "integer",
        "description": "The second value.",
      },
    },
    "required": ["a", "b"],
    "type": "object",
  }
  "#
  ```

**3. MCP stdio tools**

  `command`: fill in the command to be called

  `args`: fill in parameters

  ```
  mcp_servers: [
    StdIoServer(
      command: "./rust-mcp-filesystem",
      args: [
        "--allow-write",
        "./",
      ],
    ),
    StdIoServer(
      command: "uvx",
        args: [
          "excel-mcp-server",
          "stdio",
        ],
    ),
  ]
  ```

For complex tasks, you can activate the `plan mode` (only valid when calling tools), which will first create a plan, break down the problem into multiple subtasks, and then complete them one by one. Each step will be judged based on the previously completed steps, whether to proceed to the next step or update the plan. If the task exceeds the capabilities of the model and specified tools, it will end directly and return the reason.

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/plan_mode.png" width="50%">

## 🍔 Summarize and compress historical messages
Too many historical messages can take up valuable context. If previous messages are unrelated to recent tasks, you can use `contextual messages` to limit the number of historical messages included in each question, or click the delete button above the message box to delete them. But if there are many historical messages related to the current tasks, you can click the summary button (<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/format-space-less-svgrepo-com.svg" width="18" height="18" align="center">) in the bottom left corner of the page to summarize and compress the historical messages within the specified range of `contextual messages`. This not only preserves the previous historical message information, but also reduces the use of context.

## 📺 Detailed Instructions
[YouTube demo vedio](https://youtu.be/c1DeuIodiSk)

[bilibili demo vedio](https://www.bilibili.com/video/BV1bBuzzAEXs)

[Chinese manual](https://github.com/jingangdidi/chatsong/blob/main/doc/manual_zh.md)

[English manual](https://github.com/jingangdidi/chatsong/blob/main/doc/manual_en.md)

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/screenshot-en-label.png">

This section remains pending completion and will be duly supplemented.

## 🧬 message and Q&A pair
- One message is a single question or answer.
- One Q&A pair can contain multiple messages (multiple consecutive questions + multiple consecutive answers)
<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/QA-pair.png">

## ⌨️ Building from source
```
git clone https://github.com/jingangdidi/chatsong.git
cd chatsong
cargo build --release
```

## 🚥 Arguments
```
Usage: chatsong [-c <config>] [-a <addr>] [-p <port>] [-e <engine-key>] [-s <search-key>] [-g <graph>] [-m <maxage>] [-r] [-l] [-o <outpath>]

server for LLM api

Options:
  -c, --config      config file, contain api_key, endpoint, model name
  -a, --addr        ip address, default: 127.0.0.1
  -p, --port        port, default: 8080
  -e, --engine-key  search engine key, used for google search
  -s, --search-key  search api key, used for google search
  -g, --graph       graph file, default: search for the latest *.graph file in the output path
  -m, --maxage      cookie max age, default: 1DAY, support: SECOND, MINUTE, HOUR, DAY, WEEK
  -r, --share       allow sharing of all chat logs
  -l, --english     chat page show english
  -o, --outpath     output path, default: ./chat-log
  -h, --help        display usage information
```

## 📝 config.txt
```
(
    ip_address: "127.0.0.1", // required, if you want to access from other computers within the intranet, you need to change it to the local IP address, such as 192.168.1.5
    port: 8080,              // required
    google_engine_key: "",   // optional, used for web search
    google_search_key: "",   // optional, used for web search
    allowed_path: "./",      // optional, allowed path for tools, multiple paths separated by commas, default: ./
    maxage: "1DAY",          // required, cookie maxage, support: SECOND, MINUTE, HOUR, DAY, WEEK
    show_english: true,      // required, true: show english page，false: show chinese page
    outpath: "./chat-log",   // required, where to save chat log files
    model_config: [
        Config(
            provider: "openai",          // required
            api_key: "sk-xxx",           // required
            endpoint: "https://api.xxx", // required
            models: [
                Model(
                    name: "gpt-4.1-mini-2025-04-14",          // required
                    pricing: "(in: 0.0028/k, out: 0.0112/k)", // optional
                    discription: "OpenAI gpt-4.1 model",      // optional
                    group: "gpt-4.1",                         // required
                    is_default: false,                        // required
                    is_cot: false,                            // required, does it support Chain of thought (CoT) deep reasoning
                ),
                Model(
                    name: "gpt-4.1-nano-2025-04-14",
                    pricing: "(in: 0.0007/k, out: 0.0028/k)",
                    discription: "OpenAI gpt-4.1 model",
                    group: "gpt-4.1",
                    is_default: false,
                    is_cot: false,
                ),
            ],
        ),
        Config(
            provider: "claude",
            api_key: "sk-xxx",
            endpoint: "https://api.xxx",
            models: [
                Model(
                    name: "claude-3-5-sonnet-20241022",
                    pricing: "(in: 0.015/k, out: 0.075/k)",
                    discription: "claude model",
                    group: "Claude",
                    is_default: false,
                    is_cot: false,
                ),
                Model(
                    name: "claude-3-7-sonnet-20250219",
                    pricing: "(in: 0.015/k, out: 0.075/k)",
                    discription: "claude model",
                    group: "Claude",
                    is_default: false,
                    is_cot: true,
                ),
            ],
        ),
        Config(
            provider: "gemini",
            api_key: "sk-xxx",
            endpoint: "https://api.xxx",
            models: [
                Model(
                    name: "gemini-2.0-pro-exp-02-05",
                    pricing: "(in: 0.01/k, out: 0.04/k)",
                    discription: "google gemini model",
                    group: "Gemini",
                    is_default: false,
                    is_cot: false,
                ),
                Model(
                    name: "gemini-2.0-flash",
                    pricing: "(in: 0.005/k, out: 0.02)",
                    discription: "google gemini model",
                    group: "Gemini",
                    is_default: false,
                    is_cot: false,
                ),
            ],
        ),
        Config(
            provider: "deepseek",
            api_key: "sk-xxx",
            endpoint: "https://api.deepseek.com/v1",
            models: [
                Model(
                    name: "deepseek-chat",
                    pricing: "(in: 0.002/k, out: 0.008/k)",
                    discription: "deepseek new model DeepSeek-V3",
                    group: "DeepSeek",
                    is_default: true,
                    is_cot: false,
                ),
                Model(
                    name: "deepseek-reasoner",
                    pricing: "(in: 0.004/k, out: 0.016/k)",
                    discription: "deepseek new cof model DeepSeek-R1",
                    group: "DeepSeek",
                    is_default: false,
                    is_cot: true,
                ),
            ],
        ),
    ],
    prompts: [
        Prompt(
            name: "translator",
            content: "I want you to act as an English translator, spelling corrector and improver. I will speak to you in any language and you will detect the language, translate it and answer in the corrected and improved version of my text, in English. I want you to replace my simplified A0-level words and sentences with more beautiful and elegant, upper level English words and sentences. Keep the meaning same, but make them more literary. I want you to only reply the correction, the improvements and nothing else, do not write explanations.",
        ),
        Prompt(
            name: "Rewrite to Rust",
            content: "Rewrite the following code in Rust.",
        ),
    ],
    external_tools: [
        SingleExternalTool(
            name: "complement_DNA_or_RNA",
            command: "./complement-linux-x86_x64-musl",
            description: "Calculate complement of given DNA or RNA",
            schema: r#"
{
    "properties": {
        "seq": {
            "type": "string",
            "description": "DNA or RNA sequence.",
        },
        "revcomp": {
            "type": "boolean",
            "description": "Whether to obtain the reverse complementary sequence. If present, enables reverse complementation.",
        },
        "rna": {
            "type": "boolean",
            "description": "Whether to use RNA alphabet.",
        },
    },
    "required": ["seq"],
    "type": "object",
}
"#,
        ),
        SingleExternalTool(
            name: "add_two_value",
            command: "python",
            args: ["add_two_value.py"],
            description: "add two value",
            schema: r#"
{
    "properties": {
        "a": {
            "type": "integer",
            "description": "The first value.",
        },
        "b": {
            "type": "integer",
            "description": "The second value.",
        },
    },
    "required": ["a", "b"],
    "type": "object",
}
"#,
        ),
    ],
    mcp_servers: [
        StdIoServer(
            command: "./rust-mcp-filesystem",
            args: [
                "--allow-write",
                "./",
            ],
        ),
        StdIoServer(
            command: "uvx",
            args: [
                "excel-mcp-server",
                "stdio",
            ],
        ),
    ]
)
```

## ⏰ changelog
- [2025.12.?] release [v0.4.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.4.0)
  - ⭐️Add: Add built-in filesystem tools.
  - ⭐️Add: Support the use of custom external tools, specified through `SingleExternalTool` in `config.txt`.
  - ⭐️Add: Support the use of MCP stdio tools, specified through `StdIoServer` in `config.txt`.
  - ⭐️Add: When calling tools, chatsong supports the planning mode, which first breaks down complex problems into multiple small tasks, and then call tools to implement them one by one.
  - ⭐️Add: Add a button to summarize the current history (bottom left corner of the page)
  - 💪🏻Optimize: Enlarge the question input box.
  - 💪🏻Optimize: When selecting models and tools from the dropdown menu, use clearer grouping.
- [2025.11.06] release [v0.3.3](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.3)
  - 🛠Fix: When use streaming, if there is no error in obtaining the response but the choices are empty, the answer will not be sent to the client, and a message box for the answer will not be created on the left side of the page. The client messages number will be 1 less than the server, resulting in an error in the next question. Before ending the streaming answer, check if the total string of the answer is empty. If it is empty, send "no response result" as the answer.
  - ⭐️Add: Support Qwen3-vl api, you can send images (png, jpg, jpeg) or PDF documents (automatically converting each page into an image, note that the file extension must be lowercase (.pdf), otherwise only textual content will be extracted) for inquiry. You can use the officially provided Qwen3-VL model or run [llama.cpp](https://github.com/ggml-org/llama.cpp) locally.
  - 💪🏻Optimize: change command line info `Running on http://127.0.0.1:8080` to `Running on http://127.0.0.1:8080/v1`
- [2025.10.15] release [v0.3.2](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.2)
  - 🛠Fix: Fix the issue where other computers on the intranet are not accessible.
  - 🛠Fix: The abbreviation for the chain of thought model in config.txt is spelled incorrectly, changing "cof" to "cot".
  - ⭐️Add: The command line supports "-h", previously only "--help" could be used.
  - ⭐️Add: Support calling the official API of the Z.AI GLM model. Currently, the official APIs of Deepseek, QWEN, Z.AI GLM, and Moonshot Kimi can all be called.
  - 💪🏻Optimize: The default value for "contextual messages" on the left side of the page has been changed from "unlimit" to "prompt + 1 Q&A pair".
- [2025.08.11] release [v0.3.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.1)
  - 🛠Fix: If the "stop" button is clicked while a response is in progress, the next input will be appended to the end of the incomplete answer. Switch "cancel" to "abort" ensures the server promptly detects the termination signal and ceases responding.
  - 🛠Fix: When navigating back to the previous chat history page, if any messages have been deleted from the prior records, all subsequent messages following the deleted content will not be displayed due to the discontinuity between server-side IDs and their frontend counterparts.
- [2025.07.15] release [v0.3.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.0)
  - ⭐️Add: Support delete message.
  - ⭐️Add: Support incognito mode.
  - 💪🏻Optimize: Place the upload file button on the left side of the input box.
  - 💪🏻Optimize: Place the download button and usage button in the bottom left corner of the page.
- [2025.07.11] release [v0.2.2](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.2)
  - 🛠Fix: When saving chat history by clicking the left-side button, consecutive unanswered questions at the end should no longer be removed. Previously, this caused ID mismatches between the server and the page when continuing the conversation, resulting in errors.
  - 🛠Fix: When syncing chat history across different computers, if continuing a conversation on Computer A based on a chat from Computer B, the chat history would fail to save due to path discrepancies upon closing the service.
  - ⭐️Add: Auto-scroll pauses when scrolling upward and resumes when scrolling downward.
  - ⭐️Add: Support for line breaks in input questions using Shift + Enter.
  - ⭐️Add: Display token count for uploaded files. If the file is an image or audio, the token count will not be shown.
  - 💪🏻Optimize: Command logs now use LocalTime (e.g., 2025-07-07T13:33:48.032687+08:00) instead of the default UTC time.
  - 💪🏻Optimize: The command line now displays both the current question number and its corresponding QA pair index. Previously, it only showed the question number. 
- [2025.07.07] release [v0.2.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.1)
  -  🛠Fix: When copying a newly posed question or freshly received answer (excluding prior conversation history) by clicking the avatar, the input field not automatically gains focus.
  -  🛠Fix: Token consumption not updating upon query submission.
  -  🛠Fix: Lack of response in non-streaming output scenarios.
  -  ⭐️Add: When using web search, prefix the timestamp of the query with 🌐 to indicate an web search was performed.
  -  ⭐️Add: Upon hovering over the message box, display the message number, Q&A pair number and token count for this message.
- [2025.07.01] release [v0.2.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.0)
  - Fix the issue of excessive memory usage by optimizing code highlighting.
  - Optimize contextual messages, support Q&A pair.
  - When there is no input question and the last message is an answer, the question will be asked again based on the last question.
  - There are too many parameters on the left side of the page, so the infrequently used ones will be placed separately on the "back". The left parameter area can be flipped by clicking the bottom left button, and the main commonly used parameters will be displayed on the "front" by default.
  - Add a schematic diagram of Q&A pairs by [Excalibraw](https://excalidraw.com)
- [2025.06.30] release [v0.1.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.1.1)
- [2025.06.20] release [v0.1.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.1.0)
