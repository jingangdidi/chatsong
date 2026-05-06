# chatsong
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](https://github.com/jingangdidi/chatsong/blob/main/LICENSE)

[English readme](https://github.com/jingangdidi/chatsong/blob/main/README.md)

**A lightweight(~10M), portable executable for invoking LLM with multi-API support - eliminating installation requirements while maintaining operational efficiency.**

**轻量级大语言模型OpenAI格式api调用工具，无需安装，仅一个~10M可执行文件，支持自定义多种模型（OpenAI、Claude、Gemini、DeepSeek等，以及第三方提供的api）和prompt。**

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/demo.png">

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
- ✨ 支持markdown显示和代码高亮
- 📊 支持统计每个对话的token用量（页面左下），以及每条信息的token数（鼠标停在消息框内）
- 💰 支持设置每次提问包含多少条上下文信息，极大的节省token用量
- ✂️ 支持删除问题或回答
- 😎 支持无痕模式
- 📡 支持调用Deepseek、Qwen、智谱GLM、月之暗面Kimi等兼容OpenAI格式的api
- 🔧 支持内置文件系统工具（读写文件、压缩解压等）
- 🔨 支持调用自定义的外部工具、MCP的stdio工具
- 🤔 支持计划模式，复杂问题先制定计划，再调用工具逐个实现
- 🧰 支持skills
- 👾 支持Discord机器人

## 🚀 使用示例
**目录结构**
```
你的路径
├─ chatsong   # 单个可执行文件
├─ config.txt # 参数文件，填写自己要用的模型、api-key、api地址、prompt等
├─ skills     # 存储skills的路径（可选）
└─ chat-log   # 问答记录的保存路径
```
**1. 下载预编译的可执行文件**

[latest release](https://github.com/jingangdidi/chatsong/releases)

**2. 准备config.txt**

填写自己要用的模型，以及api key、api地址等，参考[config_template.txt](https://github.com/jingangdidi/chatsong/blob/main/config_template.txt)

**3. 开启服务**

本机调用
```
./chatsong
```
如果要在内网电脑A开启服务，电脑B访问，电脑A开启服务时需指定自身的ip地址，不能是默认的127.0.0.1。
可通过命令行参数`-a <ip>`指定，例如电脑A的IP是`192.168.1.5`：
```
./chatsong -a 192.168.1.5
```
也可以直接写在参数文件`config.txt`中：
```
ip_address: "192.168.1.5",
```

**3. 浏览器访问页面**

[http://127.0.0.1:8080/v1](http://127.0.0.1:8080/v1)

[http://192.168.1.5:8080/v1](http://192.168.1.5:8080/v1)

**4. 关闭服务**
```
按下`Ctrl+C`将自动保存所有问答记录等信息至输出路径，下次开启服务可基于之前的问答继续提问。
```

## 🧰 skills
从`v0.4.2`开始支持调用skills，可以将skill文件夹直接放在skills文件夹中，也可以将多个skill文件夹放在skills文件夹下的同一文件夹中，它们会被归为一组，页面左侧下拉可以按组选择：
```
skills
├─ skill-1
├─ skill-2
├─ skill-3
└─ skill-group-a # 这个文件夹可以存储多个skills，归为同一组
   ├─ skill-a1
   ├─ skill-a2
   └─ skill-a3
```

## 🛠 调用工具
从`v0.4.0`开始支持调用工具，除了内置的文件系统工具，还可以通过`config.txt`的`SingleExternalTool`和`StdIoServer`指定自己的外部工具和MCP的stdio工具。

在页面左侧的`调用工具`下拉选项中：

  - 白色⚪表示不使用任何工具
  - 红色🔴表示选择所有工具
  - 绿色🟢表示选择内置工具
  - 紫色🟣表示选择所有自定义的外部工具
  - 黄色🟡表示选择MCP工具
  - 其他选项表示单选一个工具

在`config.txt`中添加工具：

**1. 内置工具**

  这些工具已经编译在`chatsong`内，不需要额外配置`config.txt`，可直接调用

**2. 自己的外部工具**

  `command`填写要调用的命令，`args`填写脚本以及其他参数，`description`填写该工具的功能，模型会据此判断是否使用该工具来完成某项任务，`approval`表示调用该工具是否需要用户确认
  ```
  external_tools: [
    SingleExternalTool(
      name: "工具1名称",
      command: "工具1调用的程序，例如：./my_tool.exe",
      description: "工具1的功能描述",
      approval: false,
      schema: r#"json格式参数说明"#,
    ),
    SingleExternalTool(
      name: "工具2名称",
      command: "工具2调用的程序，例如：python3",
      args: ["脚本和其他参数在这个列表中指定，例如：my_tool.py"],
      description: "工具2的功能描述",
      approval: true,
      schema: r#"json格式参数说明"#,
    )
  ]
  ```
  注意`schema`填写json格式的参数类型及说明，由于会含有`"`和换行，因此放在`r#"`和`"#`之间，例如下面示例是自己写的一个python脚本，用来计算2个数的加和，第一个参数是`--a`指定第一个数，第二个参数是`--b`指定第二个数，`type`指定参数类型，`description`描述该参数的作用：
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

**3. MCP的stdio工具**

  `command`填写要调用的命令，`args`填写参数，例如：
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

对于复杂任务，可以开启`计划模式`（仅在调用工具时有效），会先制定计划，将问题拆分为多个子任务，然后逐个完成。每一步都会基于之前已完成的步骤进行判断，继续下一步还是更新计划。如果任务超出模型和指定工具的能力范围，则会直接结束，并返回原因。

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/plan_mode.png" width="50%">

## 🍔 总结历史记录
太多的历史消息会占用宝贵的上下文，如果早前的消息与最近的问题无关，可以使用`上下文消息数`限制每次提问时包含的历史消息数量，也可以点击消息框上方的删除按钮将其删除。但如果历史记录很多，又都与当前问题相关，则可以点击页面左下角的总结按钮（<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/format-space-less-svgrepo-com.svg" width="18" height="18" align="center">），对指定`上下文消息数`范围内的历史记录进行总结压缩，这样既保留了之前的历史记录信息，有减少了上下文占用。

## 📺 详细示例
[YouTube示例视频](https://youtu.be/c1DeuIodiSk)

[bilibili示例视频](https://www.bilibili.com/video/BV1bBuzzAEXs)

[中文说明](https://github.com/jingangdidi/chatsong/blob/main/doc/manual_zh.md)

[英文说明](https://github.com/jingangdidi/chatsong/blob/main/doc/english_demo.md)

<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/screenshot-zh-label.png">

该部分会继续补充添加

## 🧬 消息（message）和问答对（Q&A pair）
- 一条消息是指：单独的一个问题或答案
- 一对问答是指：一个或连续的多个问题，加上一个或连续的多个答案，一对问答包含至少2条消息
<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/QA-pair.png">

## ⌨️ 从源码编译
```
git clone https://github.com/jingangdidi/chatsong.git
cd chatsong
cargo build --release
```
如果使用`-k`代码补全，编译时需加上`--features code_completion`:
```
cargo build --release --features code_completion
```

## 🚥 命令行参数
```
Usage: chatsong [-c <config>] [-a <addr>] [-p <port>] [-e <engine-key>] [-s <search-key>] [-C <channels>] [-w <allowed-path>] [-g <graph>] [-m <maxage>] [-r] [-l] [-A] [-k] [-S <skills>] [-b <bgc>] [-o <outpath>]

server for LLM api

Options:
  -c, --config        config file, contain api_key, endpoint, model name
  -a, --addr          ip address, default: 127.0.0.1
  -p, --port          port, default: 8080
  -e, --engine-key    search engine key, used for google search
  -s, --search-key    search api key, used for google search
  -C, --channels      channel, multiple channels separated by `::`, currently only supports discord: `-C discord:token:guild_id`
  -w, --allowed-path  allowed path, used for call tools, multiple paths separated by commas, default: ./
  -g, --graph         graph file, default: search for the latest *.graph file in the output path
  -m, --maxage        cookie max age, default: 1DAY, support: SECOND, MINUTE, HOUR, DAY, WEEK
  -r, --share         allow sharing of all chat logs
  -l, --english       chat page show english
  -A, --approval-all  approval to call all tools without pop-up prompts
  -k, --shortcut-key  enable shortcut key code complete, can be used in any editor, support 4 modes: 1. press the Left Ctrl/command 3 times (code completion), 2. press the Right Ctrl/command 3 times (write code), 3. press Left Shift 4 times (debug), 4. press Right Shift 4 times (shell command)
  -S, --skills        skills path, default: ./skills
  -b, --bgc           background color, support specify hex color or built-in colors: 1(#E6E6E6), 2(#F5F5DC), 3(#FFFFE0), 4(#E6E6FA), default: 1
  -o, --outpath       output path, default: ./chat-log
  -h, --help          display usage information
```

## 📝 config.txt
```
(
    ip_address: "127.0.0.1",       // 必填，如果要在内网的其他电脑访问，需改为本机的ip地址，比如192.168.1.5
    port: 8080,                    // 必填
    google_engine_key: "",         // 可以空着，网络搜索时要用
    google_search_key: "",         // 可以空着，网络搜索时要用
    allowed_path: "./",            // 可以空着，调用工具时允许读写的路径，多个路径用英文逗号间隔，默认当前路径
    maxage: "1DAY",                // 必填，cookie的maxage，支持：SECOND, MINUTE, HOUR, DAY, WEEK
    show_english: true,            // 必填，true表示英文页面，fasle表示中文页面
    skills_path: Some("./skills"), // skills路径，可选，不使用skills则填写None
    bgc: "1",                      // 页面背景颜色，支持hex颜色（例如#F5F5DC、#fff、#000），或使用内置的4种浅色背景：1(#E6E6E6)、2(#F5F5DC)、3(#FFFFE0)、4(#E6E6FA)，默认1
    outpath: "./chat-log",         // 必填，问答记录的保存路径
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
                    is_default: false,                        // 必填，是否作为默认模型
                    is_cot: false,                            // 必填，是否支持CoT（Chain of thought）深度推理
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
            approval: false,
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
            approval: false,
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

## ⏰ 更新记录
- [2026.05.06] release [v0.5.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.5.0)
  - ⭐️增加：增加通过监听指定快捷键，在任意编辑器使用代码补全、写代码、debug、编写shell命令，支持4种模式：1. 连按3次左侧`Ctrl`(macos是`command`)键对选中的代码进行代码补全，2. 连按3次右侧`Ctrl`(macos是`command`)键根据选中的要求编写代码，3. 连按4次左侧`Shift`键修复选中的代码，4. 连按4次右侧`Shift`键，补全当前命令行的shell命令或写出符合当前命令行命令描述的shell命令
- [2026.04.29] release [v0.4.2](https://github.com/jingangdidi/chatsong/releases/tag/v0.4.2)
  - 🛠修复：调用工具错误时未尝试3次而直接退出
  - ⭐️增加：增加支持`skills`，通过命令行`-S`参数或参数文件`skills`指定skills的路径，默认`./skills`，可以将skill直接放在skills文件夹中，也可以将多个skill文件夹放在skills文件夹下的同一文件夹中，它们会被归为一组，页面左侧下拉可以按组选择
  - ⭐️增加：内置工具增加`codebase`、`web`、`run_x`
  - ⭐️增加：增加支持Discord机器人
  - 💪🏻优化：命令行输出每次调用的详细token统计：输入token（包括cached）和输出token（包括思考的token数）
  - 💪🏻优化：支持修改页面背景色，可通过命令行`-b`参数或参数文件`bgc`指定页面背景颜色，可以指定hex颜色（例如：#F5F5DC、#fff、#000），或内置的4个颜色：1(#E6E6E6), 2(#F5F5DC), 3(#FFFFE0), 4(#E6E6FA)，默认1
  - 💪🏻优化：页面支持显示数学公式
  - 💪🏻优化：支持关闭思考（deepseek、qwen、kimi、glm）
  - 💪🏻优化：提取思考部分内容，与最终回答分隔开（deepseek、qwen、kimi、glm、minimax）
  - 💪🏻优化：调用`edit_file`工具时，支持垂直滚动条，避免超出页面范围
  - 💪🏻优化：优化页面markdown展示
- [2026.03.17] release [v0.4.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.4.1)
  - 🛠修复：内置工具`tail_file`和`read_file`。
  - 🛠修复：调用工具时序号始终为1。
  - ⭐️增加：调用内置工具`create_directory`、`edit_file`、`move_file`、`unzip_file`、`write_file`、`zip_directory`、`zip_files`前，会先弹窗要求用户确认是否继续。
  - ⭐️增加：自定义的外部工具`SingleExternalTool`增加`approval`，调用该工具前是否需要弹窗确认。
  - ⭐️增加：`-A`参数，调用所有工具时都不弹窗确认。
  - 💪🏻优化: 计划模式prompt优化，使整个流程更健壮。
  - 💪🏻优化: 调用`edit_file`时更好的显示修改的差异。
- [2026.01.01] release [v0.4.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.4.0)
  - ⭐️增加: 增加内置的文件系统工具，包含读写文件、压缩解压等
  - ⭐️增加: 支持使用自定义的外部工具，在config.txt中通过SingleExternalTool指定
  - ⭐️增加: 支持使用MCP的stdio工具，在config.txt中通过StdIoServer指定
  - ⭐️增加: 调用工具时支持计划模式，先把复杂问题拆分为多个小任务，再调用工具逐个实现
  - ⭐️增加: 增加总结当前历史记录的按钮（页面左下角）
  - ⭐️增加: 页面左侧背面增加自定义`top-p`参数
  - 💪🏻优化: 将问题输入框放大
  - 💪🏻优化: 下拉选择模型和工具时，更清晰的分组
  - 💪🏻优化: token使用量直接从模型返回的usage获取，而不是基于tiktoken估算
- [2025.11.06] release [v0.3.3](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.3)
  - 🛠修复：流式输出时，如果获取response无报错，但choices为空，则不会向前端页面发送答案，页面左侧不会创建回答的消息框，客户端消息数会比服务端少1，导致下次提问报错。在结束流式回答前，判断下回答的总字符串是否为空，如果为空，则发送“no response result”作为答案。
  - ⭐️增加：支持调用Qwen3-vl的api，发送图片（png、jpg、jpeg）或PDF文件（会自动将每页转为图片，注意格式后缀必须是小写`.pdf`，否则仅提取文本内容）进行提问。如果发送一篇pdf论文，每页大约占用1000个token，可以把最后引用文献那几页删掉以节省token。可以使用千问官方提供的Qwen3-VL的api，也可以使用[llama.cpp](https://github.com/ggml-org/llama.cpp)通过`llama-server`本地部署。
  - 💪🏻优化：命令行显示的第一条信息`Running on http://127.0.0.1:8080`改为`Running on http://127.0.0.1:8080/v1`
- [2025.10.15] release [v0.3.2](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.2)
  - 🛠修复：内网其他电脑不可访问的问题
  - 🛠修复：config.txt中思维链模型简写拼写错误，“cof”改为“cot”，即“chain of thought”。
  - ⭐️增加：命令行支持“-h”，之前只能使用“--help”。
  - ⭐️增加：支持质谱GLM模型官方api的调用，目前deepseek、qwen、智谱glm、月之暗面kimi的官方api均可调用。
  - 💪🏻优化：页面左侧“上下文消息数”默认值由之前的“不限制”改为“prompt + 1对Q&A”。
- [2025.08.11] release [v0.3.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.1)
  - 🛠修复：正在回答时如果点击stop按钮，输入的下一个问题会显示在最后一条未完成的答案后面。因为js使用cancel停止接收不会立即停止，服务端未监测到停止信号，仍继续发送，改为使用abort，服务端会立即接收到停止信号，停止回答。
  - 🛠修复：跳转到之前chat记录页面时，如果之前记录有删除信息，则删除信息之后的信息都不显示，因为服务端id不连续，与前端id不对应。
- [2025.07.15] release [v0.3.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.3.0)
  - ⭐️增加：支持删除指定问题或回答。
  - ⭐️增加：增加无痕模式（页面左下角按钮），在当前对话随时开启或关闭，决定关闭服务时chat记录保存至本地还是直接舍弃。开启无痕模式时，刷新页面或关闭后重新打开该页面，都将丢弃对话记录。
  - 💪🏻优化：上传文件按钮放到输入框左侧。
  - 💪🏻优化：下载按钮和使用说明按钮放到页面左下角。
- [2025.07.11] release [v0.2.2](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.2)
  - 🛠修复：点击页面左侧按钮保存chat记录时，不需要去除最后连续的未回答的问题，否则继续提问时服务端与页面的id不对应报错。
  - 🛠修复：不同电脑间同步chat记录，在A电脑基于B电脑的某个对话继续提问时，最后关闭服务因为路径不同导致对话记录保存失败。
  - ⭐️增加：鼠标向上滚动则停止自动向下滚动，鼠标向下滚动则恢复自动向下滚动。
  - ⭐️增加：输入问题支持shift+enter换行。
  - ⭐️增加：显示上传文件的token数，如果上传的是图片或音频，则不显示token数。
  - 💪🏻优化：命令打印的时间使用LocalTime，例如：`2025-07-07T13:33:48.032687+08:00`，之前默认使用的是UTC时间。
  - 💪🏻优化：命令行显示当前用户输入的第几条问题，以及属于第几对QA，之前只显示用户输入的第几条问题。
- [2025.07.07] release [v0.2.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.1)
  - 🛠修复：新发送的问题或新得到的答案（非之前的问答记录）点击头像复制后，不会自动focus到输入框。
  - 🛠修复：发送问题后左侧“输入的总token”没有实时更新，而是回答完成后才更新。
  - 🛠修复：非流式输出时无响应。
  - ⭐️增加：如果使用网络搜索，则在该问题消息框上面的时间前加上🌐，表示该问题进行了网络搜索。
  - ⭐️增加：鼠标停在消息框上时，显示当前问题或答案是第几条message，第几对Q&A，以及该问题或答案的token数。
- [2025.07.01] release [v0.2.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.2.0)
  - 修复问答信息太多时，频繁调用代码高亮导致内存占用增加的问题。
  - 优化左侧上下文参数选项，支持根据Q&A问答对进行限制。
  - 当没有输入问题，最后一条消息是回答时，此时直接发起提问，会基于最后一个问题再问一次。
  - 页面左侧参数太多，将不常用的单独放到“背面”，通过左下按钮可切换左侧参数区的翻转，默认将主要常用的参数展示在“正面”。
  - 添加Q&A问答对示意图，使用[Excalibraw](https://excalidraw.com)绘制。
- [2025.06.30] release [v0.1.1](https://github.com/jingangdidi/chatsong/releases/tag/v0.1.1)
- [2025.06.20] release [v0.1.0](https://github.com/jingangdidi/chatsong/releases/tag/v0.1.0)
