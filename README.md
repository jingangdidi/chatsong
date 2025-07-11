# chatsong
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/jingangdidi/chatsong/blob/main/LICENSE)

[中文文档](https://github.com/jingangdidi/chatsong/blob/main/README_zh.md)

**A lightweight(~10M), portable executable for invoking LLM with multi-API support - eliminating installation requirements while maintaining operational efficiency.**

**轻量级大语言模型api调用工具，无需安装，仅一个~10M可执行文件，支持自定义多种模型（OpenAI、Claude、Gemini、DeepSeek等，以及第三方提供的api）和prompt。**

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/screenshot.png">

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
```
./chatsong
```
**4. visit directly in your browser**

[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)

**5. terminate the service**
```
press `Ctrl+C` to automatically save all chat records to the output directory while simultaneously updating the graph file
```

## 📺 Detailed Instructions
[YouTube demo vedio](https://youtu.be/e-ONlqtLmMk)

[bilibili demo vedio](https://www.bilibili.com/video/BV17m3ezBEz6)

This section remains pending completion and will be duly supplemented.

## 🧬 message and Q&A pair
- One message is a single question or answer.
- One Q&A pair can contain multiple messages (multiple consecutive questions + multiple consecutive answers)
<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/QA-pair.png">

## 🛠 Building from source
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
  --help, help      display usage information
```

## 📝 config.txt
```
(
    ip_address: "127.0.0.1", // required
    port: 8080,              // required
    google_engine_key: "",   // optional, used for web search
    google_search_key: "",   // optional, used for web search
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
                    is_cof: false,                            // required
                ),
                Model(
                    name: "gpt-4.1-nano-2025-04-14",
                    pricing: "(in: 0.0007/k, out: 0.0028/k)",
                    discription: "OpenAI gpt-4.1 model",
                    group: "gpt-4.1",
                    is_default: false,
                    is_cof: false,
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
                    is_cof: false,
                ),
                Model(
                    name: "claude-3-7-sonnet-20250219",
                    pricing: "(in: 0.015/k, out: 0.075/k)",
                    discription: "claude model",
                    group: "Claude",
                    is_default: false,
                    is_cof: true,
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
                    is_cof: false,
                ),
                Model(
                    name: "gemini-2.0-flash",
                    pricing: "(in: 0.005/k, out: 0.02)",
                    discription: "google gemini model",
                    group: "Gemini",
                    is_default: false,
                    is_cof: false,
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
                    is_cof: false,
                ),
                Model(
                    name: "deepseek-reasoner",
                    pricing: "(in: 0.004/k, out: 0.016/k)",
                    discription: "deepseek new cof model DeepSeek-R1",
                    group: "DeepSeek",
                    is_default: false,
                    is_cof: true,
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
    ]
)
```

## ⏰ changelog
- [2025.07.11] release v0.2.2
  - 🛠Fix: When saving chat history by clicking the left-side button, consecutive unanswered questions at the end should no longer be removed. Previously, this caused ID mismatches between the server and the page when continuing the conversation, resulting in errors.
  - 🛠Fix: When syncing chat history across different computers, if continuing a conversation on Computer A based on a chat from Computer B, the chat history would fail to save due to path discrepancies upon closing the service.
  - ⭐️Add: Auto-scroll pauses when scrolling upward and resumes when scrolling downward.
  - ⭐️Add: Support for line breaks in input questions using Shift + Enter.
  - ⭐️Add: Display token count for uploaded files. If the file is an image or audio, the token count will not be shown.
  - 💪🏻Optimize: Command logs now use LocalTime (e.g., 2025-07-07T13:33:48.032687+08:00) instead of the default UTC time.
  - 💪🏻Optimize: The command line now displays both the current question number and its corresponding QA pair index. Previously, it only showed the question number. 
- [2025.07.07] release v0.2.1
  -  🛠Fix: When copying a newly posed question or freshly received answer (excluding prior conversation history) by clicking the avatar, the input field not automatically gains focus.
  -  🛠Fix: Token consumption not updating upon query submission.
  -  🛠Fix: Lack of response in non-streaming output scenarios.
  -  ⭐️Add: When using web search, prefix the timestamp of the query with 🌐 to indicate an web search was performed.
  -  ⭐️Add: Upon hovering over the message box, display the message number, Q&A pair number and token count for this message.
- [2025.07.01] release v0.2.0
  - Fix the issue of excessive memory usage by optimizing code highlighting.
  - Optimize contextual messages, support Q&A pair.
  - When there is no input question and the last message is an answer, the question will be asked again based on the last question.
  - There are too many parameters on the left side of the page, so the infrequently used ones will be placed separately on the "back". The left parameter area can be flipped by clicking the bottom left button, and the main commonly used parameters will be displayed on the "front" by default.
  - Add a schematic diagram of Q&A pairs by [Excalibraw](https://excalidraw.com)
- [2025.06.30] release v0.1.1
- [2025.06.20] release v0.1.0
