# chatsong
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/jingangdidi/chatsong/blob/main/LICENSE)

[English readme](https://github.com/jingangdidi/chatsong/blob/main/README.md)

**A lightweight(~10M), portable executable for invoking LLM with multi-API support - eliminating installation requirements while maintaining operational efficiency.**

**轻量级大语言模型api调用工具，无需安装，仅一个~10M可执行文件，支持自定义多种模型（OpenAI、Claude、Gemini、DeepSeek等，以及第三方提供的api）和prompt。**

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/shortcut.png">

## 👑 特点
- 💪 单个可执行文件（~10M），无需安装
- 🔐 隐私安全，所有问答记录都本地存储，且随意删除
- 🤖 支持同时调用多种大模型（OpenAI、Claude、Gemini、DeepSeek等，以及第三方提供的api）
- 🎨​ 在config.txt中自定义要用的模型以及prompt
- 1️⃣​ 支持将问答记录保存至单个HTML文件
- 📚​ 支持在同一对话中调用不同模型
- ​🌐​ 支持网络搜索和指定url内容的提取
- ​📤​ 支持上传zip、html、pdf文件，自动解析内容, 以及常规文本文件
- 💻​ 支持调用本地部署的大模型（比如调用本地llama-server部署的大模型）
- ✨ 支持markdown显示：代码高亮、mermaid流程图
- 📊 支持统计每个对话的token用量
- 💰 支持设置每次提问包含多少条上下文信息，极大的节省token用量

## 🚀 使用示例
**目录结构**
```
你的路径
├─ chatsong   # 单个可执行文件
├─ config.txt # 参数文件，填写自己要用的模型、api-key、api地址、prompt等
└─ chat-log   # 问答记录的保存路径
```
**1. 下载预编译的可执行文件**

[latest release](https://github.com/jingangdidi/chatsong/releases)

**2. 准备config.txt**

填写自己要用的模型，以及api key、api地址等，详见[config_template.txt](https://github.com/jingangdidi/chatsong/blob/main/config_template.txt)

**3. 开启服务**
```
./chatsong
```
**3. 浏览器访问页面**

[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)

**4. 关闭服务**
```
按下`Ctrl+C`将自动保存所有问答记录等信息至输出路径，下次开启服务可基于之前的问答继续提问。
```

## 📺 详细示例
[YouTube示例视频](https://youtu.be/IOfFhxuk6Xc)

该部分会继续补充添加

## 🛠 从源码编译
```
git clone https://github.com/jingangdidi/chatsong.git
cd chatsong
cargo build --release
```

## 🚥 命令行参数
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
    ip_address: "127.0.0.1", // 必填
    port: 8080,              // 必填
    google_engine_key: "",   // 可以空着，网络搜索时要用
    google_search_key: "",   // 可以空着，网络搜索时要用
    maxage: "1DAY",          // 必填，cookie的maxage，支持：SECOND, MINUTE, HOUR, DAY, WEEK
    show_english: true,      // 必填，true表示英文页面，fasle表示中文页面
    outpath: "./chat-log",   // 必填，问答记录的保存路径
    model_config: [
        Config(
            provider: "openai",          // 必填，且不能重复
            api_key: "sk-xxx",           // 必填
            endpoint: "https://api.xxx", // 必填
            models: [
                Model(
                    name: "gpt-4.1-mini-2025-04-14",          // 必填
                    pricing: "(in: 0.0028/k, out: 0.0112/k)", // 可以空着
                    discription: "OpenAI gpt-4.1 model",      // 可以空着
                    group: "gpt-4.1",                         // 必填
                    is_default: false,                        // 必填
                    is_cof: false,                            // 必填
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
