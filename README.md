# chatsong
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/jingangdidi/chatsong/blob/main/LICENSE)

**A lightweight, portable executable for invoking LLM with multi-API support - eliminating installation requirements while maintaining operational efficiency.**

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/shortcut.png">

## Features
- 🚀 Single-file executable - no installation required
- 🔄 Unified multi-API support for LLM providers
- ⚡ Optimized for efficiency with minimal footprint
- Customize models and prompts within the config file
- 1️⃣​ Support saving Q&A records as a single HTML file
- Support invoking different models within the same conversation
- Support web search and urls
- Support upload and parse zip, html, pdf, and text file
- Support add local model in config.txt (e.g. provide by llama-server)

## Simple usage
**structure**
```
some dir
├─ chatsong   # single executable file
├─ config.txt # config file
└─ chat-log   # save chat log
```
**1. start server**
```
./chatsong
```
**2. visit directly in your browser**

[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)

**3. terminate the service**
```
press `Ctrl+C` to automatically save all chat records to the output directory while simultaneously updating the graph file
```

## Detailed Instructions
This section remains pending completion and will be duly supplemented.

## Arguments
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

## config.txt
```
(
    ip_address: "127.0.0.1",
    port: 8080,
    google_engine_key: "", # used for web search
    google_search_key: "", # used for web search
    maxage: "1DAY",        # cookie maxage, support: SECOND, MINUTE, HOUR, DAY, WEEK
    show_english: true,    # true: show english page，false: show chinese page
    outpath: "./chat-log", # where to save chat log files
    model_config: [
        Config(
            provider: "openai",
            api_key: "sk-xxx",
            endpoint: "https://api.xxx",
            models: [
                Model(
                    name: "gpt-4.1-mini-2025-04-14",
                    pricing: "(in: 0.0028/k, out: 0.0112/k)",
                    discription: "OpenAI gpt-4.1 model",
                    group: "gpt-4.1",
                    is_default: false,
                    is_cof: false,
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
