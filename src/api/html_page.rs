use crate::{
    info::{
        get_log_for_display, // 获取指定uuid最新问答记录，提取字符串，用于在chat页面显示
        get_token, // 获取指定uuid问题和答案的总token数
        get_prompt_name, // 获取当前uuid的prompt名称
        pop_message_before_end, // 在保存指定uuid的chat记录之前，先去指定uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
        DisplayInfo, // 将之前问答记录显示到页面
    },
    graph::get_all_related_uuid, // 获取与指定uuid相关的所有uuid
    parse_paras::PARAS, // 存储命令行参数的全局变量
};

/// 将svg图片编码为base64使用，注意要加上“data:image/svg+xml;base64,”前缀，notepad++设置编码为“以UTF-8无BOM格式编码”
const ICON_SHORTCUT: &str = include_str!("../../assets/image/robot-7.txt");
const ICON_USER: &str = include_str!("../../assets/image/user-icon-1.txt");
const ICON_CHATGPT: &str = include_str!("../../assets/image/robot-1.txt");
const ICON_DOWNLOAD: &str = include_str!("../../assets/image/icon_download.txt");
const ICON_UPLOAD: &str = include_str!("../../assets/image/icon_upload.txt");
const ICON_HELP: &str = include_str!("../../assets/image/icon_help.txt");
const ICON_SEND: &str = include_str!("../../assets/image/icon_send.txt");
const ICON_STOP: &str = include_str!("../../assets/image/stop-circle-svgrepo-com-3.txt");
const ICON_SETTING: &str = include_str!("../../assets/image/setting.txt");

/// 将marked.min.js下载下来，不需要每次联网加载
const MARKED_MIN_JS: &str = include_str!("../../assets/js/marked.min.js");

/// 将PrismJS代码高亮下载下来，不需要每次联网加载
const PRISM_MIN_JS: &str = include_str!("../../assets/js/Prism_min.js");
const PRISM_MIN_CSS: &str = include_str!("../../assets/css/Prism_min.css");

/// chat页面ch和en共用的css代码
const CSS_CODE: &str = include_str!("../../assets/css/style.css");

/// 下载页面用的css代码
const CSS_CODE_DOWNLOAD: &str = include_str!("../../assets/css/style_for_download.css");

/// 生成主页html字符串，css和js都写在html中
/// v: api版本，例如：`/v1`
pub fn create_main_page_ch(uuid: &str, v: String) -> String {
    // 获取当前uuid的问题和答案的总token数
    let token = get_token(uuid);
    // 获取当前uuid的prompt名称
    let prompt_name = get_prompt_name(uuid);
    // 获取与当前uuid相关的所有uuid
    let related_uuid_prompt = get_all_related_uuid(uuid);

    // 创建包含css和js，并插入chat记录的html页面
    let mut result = r###"<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="author" content="srx">
    <title>chatsong</title>
    <!-- <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/5.13.0/css/all.min.css" rel="stylesheet" /> -->
    <!-- <link href="{{ v }}/templates/css/all.min.css" rel="stylesheet" /> -->
    <!-- <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/marked.min.js"></script> -->
    <!-- <script srx="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/highlight.min.js"></script> -->
"###.to_string();
    //result += &format!("    <link rel='shortcut icon' href='{}/templates/images/robot-7.svg' type='image/x-icon'>\n", v);
    result += &format!("    <link rel='shortcut icon' href='{}' type='image/x-icon'>\n", ICON_SHORTCUT);
    result += "</head>\n";

    result += "<style type='text/css'>\n";
    result += CSS_CODE;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += PRISM_MIN_CSS;
    result += "</style>\n";

    result += r###"<body>
    <!-- setting -->
    <div id="left-part" class="side-nav">

        <!-- select create a new chat -->
        <div class="top_add_space" title="选择prompt开启新对话，“保持当前会话”表示不开启新对话，基于当前对话继续提问">
            <label>开启新会话</label>
            <select id="select-prompt" class="left_para for_focus" name="prompt">
                <option disabled>--选择开启新会话的prompt--</option>
                <option value="-1" selected>保持当前对话</option>
                <option value="0">无prompt</option>
"###;
    result += &PARAS.api.pulldown_prompt;
    result += r###"            </select>
        </div>

        <!-- 对话名称 -->
        <div class="top_add_space" title="每次开启新对话时，可以指定对话名称，这样在“相关uuid”中方便选择">
            <label>新对话名称（可选）</label>
            <input id="input-chat-name" class="left_para" type="text" name="chat-name" placeholder="chat name (optional)">
        </div>

        <!-- select model -->
        <div class="top_add_space" title="当前支持的模型，同一个对话可以使用不同模型进行提问">
            <label>模型</label>
            <select id="select-model" class="left_para for_focus" name="model">
"###;
    result += &PARAS.api.pulldown_model;
    result += r###"            </select>
        </div>

        <!-- select recent log -->
        <div class="top_add_space" title="选择每次提问包含的最多问答对或消息数量，可以节省token">
            <label>上下文消息数</label>
            <select id="select-log-num" class="left_para for_focus" name="num">
                <option disabled>--选择数量--</option>
                <option value="unlimit" selected>不限制</option>
                <option value="1qa">1对Q&A</option>
                <option value="2qa">2对Q&A</option>
                <option value="3qa">3对Q&A</option>
                <option value="4qa">4对Q&A</option>
                <option value="5qa">5对Q&A</option>
                <option value="p1qa">prompt + 1对Q&A</option>
                <option value="p2qa">prompt + 2对Q&A</option>
                <option value="p3qa">prompt + 3对Q&A</option>
                <option value="p4qa">prompt + 4对Q&A</option>
                <option value="p5qa">prompt + 5对Q&A</option>
                <option value="1">1条信息</option>
                <option value="2">2条信息</option>
                <option value="3">3条信息</option>
                <option value="4">4条信息</option>
                <option value="5">5条信息</option>
                <option value="p1">prompt + 1条信息</option>
                <option value="p2">prompt + 2条信息</option>
                <option value="p3">prompt + 3条信息</option>
                <option value="p4">prompt + 4条信息</option>
                <option value="p5">prompt + 5条信息</option>
            </select>
        </div>

        <div class="top_add_space switch-toggle" title="使用提出的问题进行网络搜索，然后基于搜索结果进行回答；或解析指定url，然后基于解析结果进行回答">
            <label>网络搜索</label>
            <input id="select-web" class="left_para for_focus" type="checkbox" name="web">
            <label for="select-web"></label>
        </div>

        <!-- other button -->
        <!-- https://fontawesome.com/icons -->
        <div id="left-down">
            <ul>
                <!-- save chat log -->
                <li title="保存当前对话html页面">
"###;
    result += &format!("                    <a href='http://{}:{}{}/save-log'>
                        <img class='para-btn' src='{}' />保存当前对话\n", PARAS.addr_str, PARAS.port, v, ICON_DOWNLOAD);
    //result += r###"                        <i class="fa fa-download"></i>Save chat log
    result += r###"                    </a>
                </li>

                <!-- upload file -->
                <li title="上传文件，支持多个文件">
                    <!-- 上传文件后保持当前页面 https://stackoverflow.com/questions/5733808/submit-form-and-stay-on-same-page -->
                    <iframe name="hiddenFrame" class="hide"></iframe>
"###;
    result += &format!("                    <form id='form' target='hiddenFrame' action='http://{}:{}{}/upload' method='post' enctype='multipart/form-data'>
                        <img class='para-btn' src='{}' />\n", PARAS.addr_str, PARAS.port, v, ICON_UPLOAD);
    //result += r###"                        <i class="fa fa-upload"></i>
    result += r###"                        <!-- 选好文件后直接提交，不需要submit按钮 https://stackoverflow.com/questions/7321855/how-do-i-auto-submit-an-upload-form-when-a-file-is-selected -->
                        <!-- <input id="upload-file" onchange="form.submit();form.reset();" type="file" name="file" multiple> -->
                        <input id="upload-file" type="file" name="file" multiple>
                        <!-- <input type="submit" value="submit"> -->
                    </form>
                </li>

                <!-- usage -->
                <li title="查看使用说明">
"###;
    result += &format!("                    <a href='http://{}:{}{}/usage'>
                        <img class='para-btn' src='{}' />使用说明\n", PARAS.addr_str, PARAS.port, v, ICON_HELP);
    //result += r###"                        <i class="fas fa-question-circle"></i>Usage
    result += r###"                    </a>
                </li>
            </ul>
        </div>

        <!-- show prompt -->
        <div class="top_add_space" title="当前对话的prompt">
            <label>当前prompt</label>
            <input id="show-prompt" class="left_para">
        </div>

        <!-- show uuid -->
        <div class="top_add_space" title="当前对话的uuid，记住该uuid，之后可再次查看并提问">
            <label>当前uuid</label>
            <input id="show-uuid" class="left_para">
        </div>

        <!-- show input token -->
        <div class="top_add_space" title="当前对话提问的总token">
            <label>输入的总token</label>
            <input id="show-in-token" class="left_para">
        </div>

        <!-- show output token -->
        <div class="top_add_space" title="当前对话回答的总token">
            <label>输出的总token</label>
            <input id="show-out-token" class="left_para">
        </div>

    </div>

    <div id="left-part-other" class="side-nav">
        <!-- select chain of thought effort -->
        <div class="top_add_space" title="选择思考的深度和是否显示思考过程，仅对CoF模型有效">
            <label>思考的深度</label>
            <select id="select-effort" class="left_para for_focus" name="effort">
                <option disabled>--选择思考的深度--</option>
                <option value="1" selected title="简单问答，显示思考过程">Low--显示思考过程</option>
                <option value="2" title="简单问答，不显示思考过程">Low--不显示思考过程</option>
                <option value="3" title="多步骤推理，显示思考过程">Medium--显示思考过程</option>
                <option value="4" title="多步骤推理，不显示思考过程">Medium--不显示思考过程</option>
                <option value="5" title="复杂逻辑推导，显示思考过程">High--显示思考过程</option>
                <option value="6" title="复杂逻辑推导，不显示思考过程">High--不显示思考过程</option>
            </select>
        </div>

        <!-- uuid -->
        <div class="top_add_space" title="输入对话的uuid，查看对话内容以及继续提问">
            <label>uuid</label>
            <input id="input-uuid" class="left_para" type="text" name="uuid" placeholder="uuid for log">
        </div>

        <!-- select related uuid -->
        <div class="top_add_space" title="与当前对话直接相关的其他对话，实现不同对话间跳转复用">
            <label>相关uuid</label>
            <select id="select-related-uuid" class="left_para for_focus" name="related-uuid">
                <option value="-1" disabled selected>--选择uuid--</option>
"###;
    for i in related_uuid_prompt {
        result += &format!("                <option value='{}'>{} ({})</option>\n", i.0, i.0, i.1);
    }
    result += r###"            </select>
        </div>

        <!-- temperature -->
        <div class="top_add_space" title="控制模型生成文本的随机性，取值范围为0~2。温度越高，生成的文本越随机、越发散；温度越低，生成的文本越保守、越集中">
            <label>温度</label>
            <input id="input-temperature" class="left_para" type="number" min="0" max="2" name="temperature" placeholder="temperature">
        </div>

        <!-- select stream -->
        <!--<div class="top_add_space" title="流式输出边生成边显示，否则得到完整答案后一次性显示全部">
            <label>流式输出</label>
            <select id="select-stm" class="left_para for_focus" name="stream">
                <option disabled>--是否流式输出--</option>
                <option value="yes" selected>Yes</option>
                <option value="no">No</option>
            </select>
        </div>-->

        <div class="top_add_space switch-toggle" title="流式输出边生成边显示，否则得到完整答案后一次性显示全部">
            <label>流式输出</label>
            <input id="select-stm" class="left_para for_focus" type="checkbox" checked name="stream">
            <label for="select-stm"></label>
        </div>

        <!-- select voice -->
        <div class="top_add_space" title="选择生成speech的音色">
            <label>声音</label>
            <select id="select-voice" class="left_para for_focus" name="voice">
                <option disabled>--选择speech声音--</option>
                <option value="1" selected>Alloy</option>
                <option value="2">Echo</option>
                <option value="3">Fable</option>
                <option value="4">Onyx</option>
                <option value="5">Nova</option>
                <option value="6">Shimmer</option>
            </select>
        </div>

    </div>

    <!-- chat part -->
    <div id="right-part" class="content">
        <!-- chat content region -->
        <div id="scrolldown" class="chat-content-area">
"###;
    let (logs_len, qa_num, logs) = get_log_for_display(uuid, true); // cookie对应的chat记录
    for (i, log) in logs.iter().enumerate() {
        if log.is_query { // 用户输入的问题
            result += &format!("            <!-- user -->
            <div class='right-time'>{}{}</div>
            <div class='user-chat-box'>
                <div class='q_icon_query'>
                    <div class='chat-txt right' id='{}' title='第{}条信息，第{}对问答，{}个token'></div>
                    <div class='chat-icon'>\n", if log.is_web {"🌐 "} else {""}, log.time, log.id, if logs_len > 0 {i+1} else {0}, log.idx_qa, log.token);
            if log.is_img || log.is_voice {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_USER);
            } else {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='点击复制' src='{}' />", log.id, ICON_USER);
            }
            result += r###"
                    </div>
                </div>
            </div>
"###;
        } else { // 答案
            result += &format!("            <!-- robot -->
            <div class='left-time'>{}</div>
            <div class='gpt-chat-box'>
                <div class='chat-icon'>\n", log.time);
            if log.is_img || log.is_voice {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_CHATGPT);
            } else {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='点击复制' src='{}' />", log.id, ICON_CHATGPT);
            }
            result += &format!("
                </div>
                <div class='chat-txt left' id='{}' title='第{}条信息，第{}对问答，{}个token'></div>
            </div>\n", log.id, if logs_len > 0 {i+1} else {0}, log.idx_qa, log.token);
        }
    }
    result += r###"        </div>

        <!-- user input region -->
        <div class="chat-inputs-container">
            <div class="chat-inputs-inner">
                <textarea autofocus name="Input your query" id="input_query" placeholder="Input your query"></textarea>
                <span id="submit_span" class="for_focus_button">
"###;
//                    <i class="search_btn fa fa-paper-plane" aria-hidden="true"></i>
    result += &format!("                    <img src='{}' class='search_btn' aria-hidden='true' />", ICON_SEND);
    result += r###"
                </span>
            </div>
        </div>

    </div>

    <!-- footer -->
    <footer>
        <button onclick='toggle()' id='left-toggle' title='切换参数栏设置'>
"###;
    result += &format!("            <img src='{}' aria-hidden='true' />", ICON_SETTING);
    result += r###"
        </button>
        <!-- <div>&copy; 2025 Copyright srx</div> -->
        <a href='https://github.com/jingangdidi'>https://github.com/jingangdidi</a>
    </footer>

    <script>
"###;
    result += &format!("{}\n", PRISM_MIN_JS);
    result += &format!("{}\n", MARKED_MIN_JS);
    result += r###"    </script>
    <script>
        // markdown转html
        function markhigh() {
"###;
    for (idx, log) in logs.iter().enumerate() {
        result += &format!("            var msg = document.getElementById('{}');
            var tmp = `{}`; // 这里将模板中的chat内容（已将“`”做了转译，“script”结束标签去掉了“<”）存入变量中
            if (tmp.startsWith('data:image/svg+xml;base64,')) {{ // 插入图片
                let tmp_img = document.createElement('img');
                tmp_img.src = tmp;
                msg.appendChild(tmp_img);\n", log.id, log.content);
        if log.is_voice {
            result += "                tmp_img.setAttribute('class', 'voice-size');\n"; // 设置voice图标大小
        }
        if !log.is_query { // 回答生成的图片或音频文件，添加hover下载按钮
            result += &format!("                let tmp_div = document.createElement('div');
                tmp_div.setAttribute('class', 'details');
                let tmp_a = document.createElement('a');
                tmp_a.setAttribute('class', 'title');
                tmp_a.setAttribute('href', 'http://{}:{}{}/save/{}');
                tmp_a.textContent = 'Download';
                tmp_div.appendChild(tmp_a);
                msg.setAttribute('class', 'chat-txt left tile'); // 加上tile
                msg.appendChild(tmp_div);\n", PARAS.addr_str, PARAS.port, v, idx);
        }
        result += r###"
            } else { // 文本问题或答案
                tmp = tmp.replaceAll('\\`', '`').replaceAll('/scrip', '</scrip'); // 恢复转译的“`”和“script”结束标签
"###;
        if log.is_query { // 用户输入的问题
            result += "                msg.textContent = tmp.replaceAll('\\\\n', '\\n');\n            }\n            // 问题不需要markdown解析\n";
        } else { // 答案
            result += "                msg.innerHTML = marked.parse(tmp).replaceAll('<p>', '').replaceAll('</p>', '');\n            }\n";
        }
    }
    result += &format!("        }}
        window.onload = markhigh();
        document.getElementById('show-prompt').value = '{}';
        document.getElementById('show-uuid').value = '{}';
        document.getElementById('show-in-token').value = '{}';
        document.getElementById('show-out-token').value = '{}';
", prompt_name, uuid, token[0], token[1]);
    result += "    </script>
</body>

<!-- js -->
<script type='text/javascript'>
";
    result += &format!("    var address = 'http://{}:{}{}/chat?q='; // http://127.0.0.1:8080\n    var current_id = {}; // 当前最新message的id，之后插入新问题或答案的id会基于该值继续增加\n    var qa_num = {}; // 问答对数量\n    var last_is_answer = true; // 最后一条信息是否是回答\n", PARAS.addr_str, PARAS.port, v, logs_len, qa_num);
    result += r###"    var emptyInput = true; // 全局变量，存储输入问题是否为空
    var no_message = true; // 是否没有获取到效回复，没有获取到，则将添加的msg_res删掉
    var already_clear_log = false; // 是否已清除了当前的记录
    var for_markdown = ''; // 累加原始信息，用于markdown显示
    // 左侧下拉菜单选取完成后，自动focus到问题输入框
    document.querySelectorAll('.for_focus').forEach(select => {
        select.addEventListener('change', function() {
            document.getElementById('input_query').focus();
        });
    });
    // 点击提交按钮和头像后，自动focus到问题输入框。由于头像消息是动态增加的，因此不能像上面那样，而应该使用事件委托
    document.addEventListener('click', function(event) {
        if (event.target.classList.contains('for_focus_button')) {
            document.getElementById('input_query').focus();
        }
    });
    // 停止接收回答
    let reader; // 接收答案
    let isStopped = true; // 是否停止接收答案
    // 左下按钮，切换左侧参数栏
    let toggleMain = true; // true显示主参数，false显示其余参数
    function toggle () {
        toggleMain = !toggleMain;
        const left_main = document.getElementById('left-part');
        const left_other = document.getElementById('left-part-other');
        if (toggleMain) {
            left_other.classList.add('animate');
            left_main.style.display = 'block';
            left_other.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_main.classList.remove('animate');
            });
        } else {
            left_main.classList.add('animate');
            left_other.style.display = 'block';
            left_main.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_other.classList.remove('animate');
            });
        }
    }
    // 上传文件，选好文件后直接提交，不需要submit按钮 https://stackoverflow.com/questions/7321855/how-do-i-auto-submit-an-upload-form-when-a-file-is-selected
    function sleep (time) {
        return new Promise((resolve) => setTimeout(resolve, time));
    }
    document.getElementById("upload-file").onchange = function(event) {
        document.getElementById("form").submit();
        for (let i = 0; i < event.target.files.length; i++) {
            const file = event.target.files[i];
            if (file) {
                insert_right_image(); // 先插入右侧的空内容，后面写入图片或上传文件的文件名
                let new_id = 'm'+(current_id-1);
                const msg_req_right = document.getElementById(new_id);
                if (file.type.startsWith('image/')) { // 插入显示上传的图片或文件名
                    // 生成临时URL并设置为图片的src
                    const objectURL = URL.createObjectURL(file);
                    let right_img = document.createElement("img");
                    right_img.src = objectURL;
                    msg_req_right.appendChild(right_img);
                } else { // 如果不是图片，显示上传文件的名称
                    msg_req_right.textContent = file.name;
                }
                sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                    scroll();
                });
            }
        }
        document.getElementById('input_query').focus();
        sleep(2000).then(() => {
            document.getElementById("form").reset();
        });
    };
    // 清空指定元素的所有子元素，https://stackoverflow.com/questions/3955229/remove-all-child-elements-of-a-dom-node-in-javascript
    function clear_all_child(id_name) {
        const parent = document.getElementById(id_name)
        while (parent.firstChild) {
            parent.firstChild.remove();
        }
    }
    // 更新相关uuid的下拉项
    function related_uuid(uuids) {
        clear_all_child('select-related-uuid');
        let options = document.getElementById("select-related-uuid");
        let disabled_option = document.createElement("option");
        disabled_option.setAttribute("value", "-1");
        disabled_option.setAttribute("disabled", "");
        disabled_option.setAttribute("selected", "");
        disabled_option.text = "--select uuid--";
        options.appendChild(disabled_option);
        for (i of uuids) {
            let uuid_option = document.createElement("option");
            uuid_option.setAttribute("value", i[0]);
            uuid_option.text = i[0]+' ('+i[1]+')';
            options.appendChild(uuid_option);
        }
    }
    // 插入右侧图片
    function insert_right_image() {
        /* 输入内容 */
        let msg_req_right = document.createElement("div");
        msg_req_right.setAttribute("class", "chat-txt right");
        let new_id = 'm'+current_id;
        current_id += 1; // id序号加1
        if (last_is_answer) {
            qa_num += 1;
            last_is_answer = false;
        }
        msg_req_right.setAttribute("title", "第"+current_id+"条信息，第"+qa_num+"对问答");
        msg_req_right.setAttribute("id", new_id);
        /* 头像 */
        let icon_div = document.createElement("div");
        icon_div.setAttribute("class", "chat-icon");
        let icon_right = document.createElement("img");
"###;
    result += &format!("        icon_right.setAttribute('src', '{}');\n", ICON_USER);
    result += r###"        icon_right.setAttribute("class", "chatgpt-icon for_focus_button");
        //icon_right.setAttribute("onclick", "copy('"+new_id+"');");
        //icon_right.setAttribute("title", "点击复制");
        icon_div.appendChild(icon_right);
        /* 提问的头像和内容放到一个div右侧对齐 */
        let q_icon_query_div = document.createElement("div");
        q_icon_query_div.setAttribute("class", "q_icon_query");
        q_icon_query_div.appendChild(msg_req_right);
        q_icon_query_div.appendChild(icon_div);
        /* 用户输入内容最外的div */
        let Con1 = document.createElement("div");
        Con1.setAttribute("class", "user-chat-box");
        /* chat区域插入输入内容和头像 */
        let message = document.getElementById("scrolldown");
        /* 提问的当前时间 */
        let timeInfo = document.createElement("div");
        timeInfo.setAttribute("class", "right-time");
        timeInfo.innerHTML = formatDate(true);
        message.appendChild(timeInfo);
        Con1.appendChild(q_icon_query_div);
        message.appendChild(Con1);
    }
    // 插入左侧答案和右侧问题
    function insert_left_right(message_content, message_time, id, is_left, is_img, is_voice, is_web, current_token) {
        if (id === current_id) { // 当前消息还没插入
            let new_id = 'm'+current_id; // 当前要插入消息的id
            current_id += 1; // id序号加1
            let msg_lr = document.createElement("div");
            msg_lr.setAttribute("id", new_id);
            if (is_img) { // 插入图片
                let lr_img = document.createElement("img");
                lr_img.src = message_content;
                msg_lr.appendChild(lr_img);
                if (is_left) { // 左侧图片hover时下载按钮
                    msg_lr.setAttribute("class", "chat-txt left tile"); // tile用于hover时下载图片或语音
                    let tmp_div = document.createElement('div');
                    tmp_div.setAttribute('class', 'details');
                    let tmp_a = document.createElement('a');
                    tmp_a.setAttribute('class', 'title');
                    if (is_voice) {
                        lr_img.setAttribute('class', 'voice-size'); // 设置voice图标大小
                    }
"###;
    result += &format!("                    tmp_a.setAttribute('href', 'http://{}:{}{}/save/'+(current_id-1));\n", PARAS.addr_str, PARAS.port, v);
    result += r###"                    tmp_a.textContent = 'Download';
                    tmp_div.appendChild(tmp_a);
                    msg_lr.appendChild(tmp_div);
                } else {
                    msg_lr.setAttribute("class", "chat-txt right");
                }
            } else {
                if (is_left) { // 文本答案
                    for_markdown = message_content.replaceAll('srxtzn', '\n');
                    msg_lr.setAttribute("class", "chat-txt left");
                    // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                    msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                    // 对每个代码块进行高亮
                    msg_lr.querySelectorAll('pre code').forEach((block) => {
                        Prism.highlightElement(block);
                    });
                } else { // 文本问题
                    msg_lr.setAttribute("class", "chat-txt right");
                    msg_lr.textContent = message_content.replaceAll('srxtzn', '\n').replaceAll('\\n', '\n'); // 不要使用innerHTML，innerHTML会识别标签将内容解析为html，textContent只是文本，innerText会受到css影响，https://stackoverflow.com/questions/31002593/type-new-line-character-in-element-textcontent
                }
            }
            /* 头像 */
            let icon_div = document.createElement("div");
            icon_div.setAttribute("class", "chat-icon");
            let icon_lr = document.createElement("img");
"###;
    result += &format!("            if (is_left) {{ // 答案
                icon_lr.setAttribute('src', '{}');
            }} else {{ // 问题
                icon_lr.setAttribute('src', '{}');
            }}
", ICON_CHATGPT, ICON_USER);
    result += r###"            icon_lr.setAttribute("class", "chatgpt-icon for_focus_button");
            if (!is_img) {
                icon_lr.setAttribute("onclick", "copy('"+new_id+"');");
                icon_lr.setAttribute("title", "点击复制");
            }
            icon_div.appendChild(icon_lr);

            /* 最外层提问/回答的当前时间 */
            let timeInfo = document.createElement("div");
            if (is_left) {
                timeInfo.setAttribute("class", "left-time");
            } else {
                timeInfo.setAttribute("class", "right-time");
            }
            if (is_web) {
                timeInfo.innerHTML = "🌐 "+message_time;
            } else {
                timeInfo.innerHTML = message_time;
            }

            /* chat区域插入问题/答案的时间 */
            let message = document.getElementById("scrolldown");
            message.appendChild(timeInfo);

            if (is_left) {
                last_is_answer = true;
                if (current_token > 0) {
                    msg_lr.setAttribute("title", "第"+current_id+"条信息，第"+qa_num+"对问答，"+current_token+"个token");
                } else {
                    msg_lr.setAttribute("title", "第"+current_id+"条信息，第"+qa_num+"对问答"); // 这里先不显示token数，等回答完成后再加上
                }
                /* 答案外的div */
                let Con2 = document.createElement("div");
                Con2.setAttribute("class", "gpt-chat-box");
                /* chat区域插入答案和头像 */
                Con2.appendChild(icon_div);
                Con2.appendChild(msg_lr);
                /* 提问的当前时间 */
                message.appendChild(Con2);
            } else {
                if (last_is_answer) {
                    qa_num += 1;
                    last_is_answer = false;
                }
                if (current_token > 0) {
                    msg_lr.setAttribute("title", "第"+current_id+"条信息，第"+qa_num+"对问答，"+current_token+"个token");
                } else {
                    msg_lr.setAttribute("title", "第"+current_id+"条信息，第"+qa_num+"对问答"); // 这里先不显示token数，等回答完成后再加上
                }
                /* 提问的头像和内容放到一个div右侧对齐 */
                let q_icon_query_div = document.createElement("div");
                q_icon_query_div.setAttribute("class", "q_icon_query");
                q_icon_query_div.appendChild(msg_lr);
                q_icon_query_div.appendChild(icon_div);
                /* 用户输入内容最外的div */
                let Con1 = document.createElement("div");
                Con1.setAttribute("class", "user-chat-box");
                Con1.appendChild(q_icon_query_div);
                message.appendChild(Con1);
            }
        } else if (id === current_id - 1) { // 当前消息已经插入，继续追加内容。由于图片base64在一个stream中，因此这里只能是stream传输的文本答案
            let new_id = 'm'+id; // 当前要插入消息的id
            let msg_lr = document.getElementById(new_id);
            for_markdown += message_content.replaceAll('srxtzn', '\n');
            // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
            msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
            // 对每个代码块进行高亮
            msg_lr.querySelectorAll('pre code').forEach((block) => {
                Prism.highlightElement(block);
            });
        } else { // 不应该出现
            console.error("message id not match: current_id='${current_id}', received_id='${id}'");
        }
    }
    // 个位数左侧加0补为2位数，https://www.toptal.com/software/definitive-guide-to-datetime-manipulation
    function pad(n) {
        return n<10 ? '0'+n : n;
    }
    // 获取当前时间，并格式化为：2024-10-20 17:37:47，https://stackoverflow.com/questions/14638018/current-time-formatting-with-javascript
    function formatDate(is_user) {
        var d = new Date();
        var year = d.getFullYear();
        var month = pad(d.getMonth()+1); // 0-11
        var day = pad(d.getDate()); // 1-31
        var hr = pad(d.getHours()); // 0-23
        var min = pad(d.getMinutes()); // 0-59
        var sec = pad(d.getSeconds()); // 0-59
        if (is_user) {
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec;
        } else {
            // https://stackoverflow.com/questions/14976495/get-selected-option-text-with-javascript
            var sel = document.getElementById("select-model");
            var text= sel.options[sel.selectedIndex].text.split(" (")[0];
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec+" "+text;
        }
    }
    // 复制指定头像id对应的内容
    function copy(id) {
        // https://code-boxx.com/strip-remove-html-tags-javascript/
        var textToCopy = document.getElementById(id).textContent;
        //console.log(textToCopy);
        navigator.clipboard.writeText(textToCopy);
    }
    /* chat region scroll bottom */
    function scroll() {
        var scrollMsg = document.getElementById("scrolldown");
        scrollMsg.scrollTop = scrollMsg.scrollHeight;
    }
    // 获取用户发起提问时提交的信息
    function get_url() {
        var req = document.getElementById("input_query").value;
        if (req !== '') { // 输入不为空才不在界面显示输入内容
            emptyInput = false;
            // 插入用户输入内容
            //insert_left_right(req, formatDate(true), current_id, false, false, false); // 不在这里插入问题，后面问题会作为MainData插入，附带token数等信息
        } else {
            emptyInput = true;
        }
        // 清空输入框，滚动到最下面，等待答案
        document.getElementById("input_query").value = "";
        scroll();
        // https://stackoverflow.com/questions/1085801/get-selected-value-in-dropdown-list-using-javascript
        // 获取选择的模型
        var para_model = document.getElementById("select-model").value;
        // 获取选择的思考深度
        var para_effort = document.getElementById("select-effort").value;
        // 获取输入的对话名称
        var para_chat_name = document.getElementById("input-chat-name").value;
        // 获取输入的uuid
        var para_uuid = document.getElementById("input-uuid").value;
        if (para_uuid === '') { // 输入的uuid优先级要高于下拉选择的uuid
            para_uuid = document.getElementById("select-related-uuid").value;
            if (para_uuid === '-1') {
                para_uuid = '';
            }
        }
        // 获取输入的temperature
        var para_temperature = document.getElementById("input-temperature").value;
        // 获取选择的stream
        var para_stm = document.getElementById("select-stm").checked;
        // 获取是否网络搜索
        var para_web = document.getElementById("select-web").checked;
        // 获取选择的要保留的最近的最多问答记录数
        var para_num = document.getElementById("select-log-num").value;
        // 使用选择的prompt开启新对话
        var para_prompt = document.getElementById("select-prompt").value;
        // 使用选择生成音频的声音
        var para_voice = document.getElementById("select-voice").value;
        // 输入框无效，并显示信息
        if (emptyInput) { // 输入为空表示提问
            var q = 0;
            document.getElementsByName('Input your query')[0].placeholder = 'Waiting for answer ...';
        } else if (para_web) { // 使用网络搜索需要等待搜索结束
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = 'Waiting for search ...';
        } else { // 输入不为空表示用户继续提问
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = 'Sending query ...';
        }
        document.getElementById('input_query').disabled = true; // 完成回复之前禁止继续提问
        // 将参数加到问题后面
        req2 = q+"&model="+para_model+"&chatname="+para_chat_name+"&uuid="+para_uuid+"&stream="+para_stm+"&web="+para_web+"&num="+para_num+"&prompt="+para_prompt+"&voice="+para_voice+"&effort="+para_effort+"&temp="+para_temperature;
        return [req, req2];
    }
    // 提交问题并获取答案
    async function send_query_receive_answer() {
        // 从服务器获取stream内容
        no_message = true;
        already_clear_log = false;
        var autoScroll = true; // 默认随着流式输出自动滚动，如果用户进行了手动滚动，则停止自动滚动，这样就保持页面停留在用户想看的那个位置
        let tmpmsg = ""; // 累加存储流式输出的结果，转为markdown
        submit_send_stop = document.getElementById("submit_span");
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' style='width: 50px;' aria-hidden='true' />\";\n", ICON_STOP);
    result += r###"        isStopped = false;
        // 由于EventSource不支持post，因此无法将问题通过body传递，只能放到url中通过url参数传递，但url有长度限制（好像大部分浏览器是2k），因此输入内容长度不能太长
        // 这里用fetch发送post，将问题字符串通过body传递，其他简单参数通过url传递
        let [req, req2] = get_url();
        const response = await fetch(address+req2, {
            method: 'POST',
            headers: {
                'Content-Type': 'text/plain;charset=UTF-8',
                'Accept': 'text/event-stream'
            },
            body: req,
        });
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = ''; // Buffer to accumulate partial messages
        // 解析数据
        while (!isStopped) {
            const { done, value } = await reader.read();
            if (done) {
                // Process any remaining data in buffer if it forms a complete message
                if (buffer.trim()) processSseBuffer(); 
                break;
            }
            buffer += decoder.decode(value, { stream: true }); // stream: true is important
            processSseBuffer();
        }
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' aria-hidden='true' />\";\n", ICON_SEND);
    result += r###"        isStopped = true;
        document.getElementById("select-prompt").value = '-1'; // prompt恢复为不开启新会话
        document.getElementById("input-chat-name").value = ''; // 清空填写的对话名称
        document.getElementById("input-uuid").value = ''; // 清空填写的uuid，此时左下“current uuid”中显示的即是填写的uuid
        document.getElementById("input_query").value = "";
        document.getElementById('input_query').disabled = false; // 已完成回复，可以继续提问
        document.getElementsByName('Input your query')[0].placeholder = 'Input your query';
        document.getElementById("input_query").focus();

        // 解析完整数据
        function processSseBuffer() {
            let eolIndex;
            // SSE messages are separated by double newlines "\n\n"
            while ((eolIndex = buffer.indexOf('\n\n')) >= 0) {
                // 从buffer中获取“\n\n”之前的内容
                const messageStr = buffer.substring(0, eolIndex);
                // 从buffer中去除“\n\n”以及之前的内容，buffer此时剩下“\n\n”之后的内容
                buffer = buffer.substring(eolIndex + 2);
                // Skip empty messages
                if (messageStr.trim() === '') continue;
                // Parse the individual SSE message
                let currentEvent = 'maindata'; // Default event type
                let currentData = [];
                // 根据\n拆分解析每行，注意一个data内不要有\n，多行可以写到多个data中
                messageStr.split('\n').forEach(line => {
                    if (line.startsWith('event: ')) {
                        currentEvent = line.substring('event: '.length).trim();
                    } else if (line.startsWith('data: ')) {
                        currentData.push(line.substring('data: '.length));
                    } else {
                        console.warn("line not starts with event and data:", line);
                    }
                });
                // 用\n将data数据合并为一个字符串
                const eventData = currentData.join('\n');
                // 基于event类型解析数据
                try {
                    const jsonData = JSON.parse(eventData);
                    switch (currentEvent) {
                        case 'metadata':
                            if (jsonData.current_token > 0) { // 回答结束，更新token数
                                let answer_id = 'm'+(current_id - 1); // 当前回答的id
                                let msg_lr = document.getElementById(answer_id);
                                const currentTitle = msg_lr.getAttribute("title");
                                msg_lr.setAttribute("title", currentTitle + "，"+jsonData.current_token+"个token");
                            }
                            //console.log('Received metadata:', jsonData);
                            // 更新页面左测当前uuid、问题token、答案token、prompt名称、相关uuid
                            document.getElementById("show-prompt").value = jsonData.prompt;
                            document.getElementById("show-uuid").value = jsonData.current_uuid;
                            document.getElementById("show-in-token").value = jsonData.in_token;
                            document.getElementById("show-out-token").value = jsonData.out_token;
                            related_uuid(jsonData.related_uuid);
                            if (autoScroll) {
                                scroll();
                            }
                            break; // 否则会继续执行下面的case
                        case 'maindata':
                            //console.log('Received maindata:', jsonData);
                            // 如果信息是之前的问答记录，先清空当前所有信息
                            if (!already_clear_log && jsonData.is_history) {
                                clear_all_child('scrolldown');
                                already_clear_log = true;
                                current_id = 0;
                                qa_num = 0;
                                last_is_answer = true;
                            }
                            // https://stackoverflow.com/questions/15275969/javascript-scroll-handler-not-firing
                            // https://www.answeroverflow.com/m/1302587682957824081
                            window.addEventListener('wheel', function() { // “scroll”无效
                                if (autoScroll) {
                                    //console.log('Scrolling via mouse');
                                    autoScroll = false; // 用户手动进行滚动，后面将不再自动滚动
                                }
                            });
                            window.addEventListener('touchmove', function() { // 触屏这个有效
                                if (autoScroll) {
                                    //console.log('Scrolling via touch');
                                    autoScroll = false; // 用户手动进行滚动，后面将不再自动滚动
                                }
                            });
                            no_message = false;
                            // 插入信息
                            if (jsonData.time_model) {
                                insert_left_right(jsonData.content, jsonData.time_model, jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                            } else { // 没有传递时间则使用当前时间
                                if (jsonData.is_left) {
                                    insert_left_right(jsonData.content, formatDate(false), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                                } else {
                                    insert_left_right(jsonData.content, formatDate(true), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                                }
                            }
                            //Prism.highlightAll();
                            if (autoScroll) {
                                if (jsonData.is_img) {
                                    sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                                        scroll();
                                    });
                                } else {
                                    scroll();
                                }
                            }
                            break; // 否则会继续执行下面的case
                        case 'close':
                            //console.log('Received close:', jsonData);
                            break; // 否则会继续执行下面的case
                        default:
                            console.log(`Received unhandled event '${currentEvent}':`, jsonData);
                    }
                } catch (e) {
                    //console.error(`Failed to parse JSON for event '${currentEvent}':`, e, 'Raw data:', eventData);
                    console.log(`Failed to parse JSON for event '${currentEvent}':`, e, 'Raw data:', eventData);
                }
            }
        }
    }
    scroll();
    // 按下回车键发送
    document.getElementById("input_query").addEventListener("keypress", async(e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            if (isStopped) { // 发送问题
                await send_query_receive_answer();
            } else { // 停止接收回答
                if (reader) reader.cancel();
                isStopped = true;
            }
        }
    });
    // 鼠标点击按钮发送
    document.getElementById("submit_span").addEventListener("click", async(e) => {
        if (isStopped) { // 发送问题
            await send_query_receive_answer();
        } else { // 停止接收回答
            if (reader) reader.cancel();
            isStopped = true;
        }
    });
</script>

</html>
"###;
    result
}

/// 生成主页html字符串，css和js都写在html中
/// v: api版本，例如：`/v1`
pub fn create_main_page_en(uuid: &str, v: String) -> String {
    // 获取当前uuid的问题和答案的总token数
    let token = get_token(uuid);
    // 获取当前uuid的prompt名称
    let prompt_name = get_prompt_name(uuid);
    // 获取与当前uuid相关的所有uuid
    let related_uuid_prompt = get_all_related_uuid(uuid);

    // 创建包含css和js，并插入chat记录的html页面
    let mut result = r###"<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="author" content="srx">
    <title>chatsong</title>
    <!-- <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/5.13.0/css/all.min.css" rel="stylesheet" /> -->
    <!-- <link href="{{ v }}/templates/css/all.min.css" rel="stylesheet" /> -->
    <!-- <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/marked.min.js"></script> -->
    <!-- <script srx="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/highlight.min.js"></script> -->
"###.to_string();
    //result += &format!("    <link rel='shortcut icon' href='{}/templates/images/robot-7.svg' type='image/x-icon'>\n", v);
    result += &format!("    <link rel='shortcut icon' href='{}' type='image/x-icon'>\n", ICON_SHORTCUT);
    result += "</head>\n";

    result += "<style type='text/css'>\n";
    result += CSS_CODE;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += PRISM_MIN_CSS;
    result += "</style>\n";

    result += r###"<body>
    <!-- setting -->
    <div id="left-part" class="side-nav">

        <!-- select create a new chat -->
        <div class="top_add_space" title="Select a "Prompt" to initiate a new conversation; choose "keep current chat" to continue with the existing dialogue without starting afresh">
            <label>start new chat</label>
            <select id="select-prompt" class="left_para for_focus" name="prompt">
                <option disabled>--select prompt--</option>
                <option value="-1" selected>keep current chat</option>
                <option value="0">no prompt</option>
"###;
    result += &PARAS.api.pulldown_prompt;
    result += r###"            </select>
        </div>

        <!-- 对话名称 -->
        <div class="top_add_space" title="Feel free to designate a specific title for each new conversation, facilitating easier selection within the "Related UUIDs" section">
            <label>new chat title (optional)</label>
            <input id="input-chat-name" class="left_para" type="text" name="chat-name" placeholder="chat name (optional)">
        </div>

        <!-- select model -->
        <div class="top_add_space" title="Currently supported models, permit the use of varying models within the same conversation for inquiries">
            <label>models</label>
            <select id="select-model" class="left_para for_focus" name="model">
"###;
    result += &PARAS.api.pulldown_model;
    result += r###"            </select>
        </div>

        <!-- select recent log -->
        <div class="top_add_space" title="Opting to include the maximum number of Q&A pairs or messages in each inquiry can conserve tokens">
            <label>contextual messages</label>
            <select id="select-log-num" class="left_para for_focus" name="num">
                <option disabled>--select number--</option>
                <option value="unlimit" selected>unlimit</option>
                <option value="1qa">1 Q&A pair</option>
                <option value="2qa">2 Q&A pairs</option>
                <option value="3qa">3 Q&A pairs</option>
                <option value="4qa">4 Q&A pairs</option>
                <option value="5qa">5 Q&A pairs</option>
                <option value="p1qa">prompt + 1 Q&A pair</option>
                <option value="p2qa">prompt + 2 Q&A pairs</option>
                <option value="p3qa">prompt + 3 Q&A pairs</option>
                <option value="p4qa">prompt + 4 Q&A pairs</option>
                <option value="p5qa">prompt + 5 Q&A pairs</option>
                <option value="1">1 message</option>
                <option value="2">2 messages</option>
                <option value="3">3 messages</option>
                <option value="4">4 messages</option>
                <option value="5">5 messages</option>
                <option value="p1">prompt + 1 message</option>
                <option value="p2">prompt + 2 messages</option>
                <option value="p3">prompt + 3 messages</option>
                <option value="p4">prompt + 4 messages</option>
                <option value="p5">prompt + 5 messages</option>
            </select>
        </div>

        <div class="top_add_space switch-toggle" title="使用提出的问题进行网络搜索，然后基于搜索结果进行回答；或解析指定url，然后基于解析结果进行回答">
            <label>web search</label>
            <input id="select-web" class="left_para for_focus" type="checkbox" name="web">
            <label for="select-web"></label>
        </div>

        <!-- other button -->
        <!-- https://fontawesome.com/icons -->
        <div id="left-down">
            <ul>
                <!-- save chat log -->
                <li title="save current chat log">
"###;
    result += &format!("                    <a href='http://{}:{}{}/save-log'>
                        <img class='para-btn' src='{}' />Save chat log\n", PARAS.addr_str, PARAS.port, v, ICON_DOWNLOAD);
    //result += r###"                        <i class="fa fa-download"></i>Save chat log
    result += r###"                    </a>
                </li>

                <!-- upload file -->
                <li title="upload your files, multiple documents are supported">
                    <!-- 上传文件后保持当前页面 https://stackoverflow.com/questions/5733808/submit-form-and-stay-on-same-page -->
                    <iframe name="hiddenFrame" class="hide"></iframe>
"###;
    result += &format!("                    <form id='form' target='hiddenFrame' action='http://{}:{}{}/upload' method='post' enctype='multipart/form-data'>
                        <img class='para-btn' src='{}' />\n", PARAS.addr_str, PARAS.port, v, ICON_UPLOAD);
    //result += r###"                        <i class="fa fa-upload"></i>
    result += r###"                        <!-- 选好文件后直接提交，不需要submit按钮 https://stackoverflow.com/questions/7321855/how-do-i-auto-submit-an-upload-form-when-a-file-is-selected -->
                        <!-- <input id="upload-file" onchange="form.submit();form.reset();" type="file" name="file" multiple> -->
                        <input id="upload-file" type="file" name="file" multiple>
                        <!-- <input type="submit" value="submit"> -->
                    </form>
                </li>

                <!-- usage -->
                <li title="review the instructions for use">
"###;
    result += &format!("                    <a href='http://{}:{}{}/usage'>
                        <img class='para-btn' src='{}' />Usage\n", PARAS.addr_str, PARAS.port, v, ICON_HELP);
    //result += r###"                        <i class="fas fa-question-circle"></i>Usage
    result += r###"                    </a>
                </li>
            </ul>
        </div>

        <!-- show prompt -->
        <div class="top_add_space" title="current prompt">
            <label>current prompt</label>
            <input id="show-prompt" class="left_para">
        </div>

        <!-- show uuid -->
        <div class="top_add_space" title="current uuid，remember this UUID, you may revisit and inquire about it at any time thereafter">
            <label>current uuid</label>
            <input id="show-uuid" class="left_para">
        </div>

        <!-- show input token -->
        <div class="top_add_space" title="The total input tokens used in the current conversation">
            <label>input token</label>
            <input id="show-in-token" class="left_para">
        </div>

        <!-- show output token -->
        <div class="top_add_space" title="The total output tokens used in the current conversation">
            <label>output token</label>
            <input id="show-out-token" class="left_para">
        </div>

    </div>
    <div id="left-part-other" class="side-nav">
        <!-- select chain of thought effort -->
        <div class="top_add_space" title="effort on reasoning for reasoning models and the visibility of the reasoning process, applicable solely to the reasoning models">
            <label>reasoning effort</label>
            <select id="select-effort" class="left_para for_focus" name="effort">
                <option disabled>--select effort--</option>
                <option value="1" selected title="favors speed and economical token usage">Low & Display the reasoning process</option>
                <option value="2" title="favors speed and economical token usage">Low & Hide the reasoning process</option>
                <option value="3" title="a balance between speed and reasoning accuracy">Medium & Display the reasoning process</option>
                <option value="4" title="a balance between speed and reasoning accuracy">Medium & Hide the reasoning process</option>
                <option value="5" title="favors more complete reasoning">High & Display the reasoning process</option>
                <option value="6" title="favors more complete reasoning">High & Hide the reasoning process</option>
            </select>
        </div>

        <!-- uuid -->
        <div class="top_add_space" title="input the UUID of the previous conversation to review its content and to proceed with your inquiry">
            <label>uuid</label>
            <input id="input-uuid" class="left_para" type="text" name="uuid" placeholder="uuid for log">
        </div>

        <!-- select related uuid -->
        <div class="top_add_space" title="implement seamless transitions and reuse across related conversations, enabling fluid navigation between distinct dialogues.">
            <label>related UUIDs</label>
            <select id="select-related-uuid" class="left_para for_focus" name="related-uuid">
                <option value="-1" disabled selected>--select uuid--</option>
"###;
    for i in related_uuid_prompt {
        result += &format!("                <option value='{}'>{} ({})</option>\n", i.0, i.0, i.1);
    }
    result += r###"            </select>
        </div>

        <!-- temperature -->
        <div class="top_add_space" title="What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic">
            <label>temperature</label>
            <input id="input-temperature" class="left_para" type="number" min="0" max="2" name="temperature" placeholder="temperature">
        </div>

        <!-- select stream -->
        <!--<div class="top_add_space" title="partial messages will be sent, like in ChatGPT">
            <label>stream</label>
            <select id="select-stm" class="left_para for_focus" name="stream">
                <option disabled>--stream--</option>
                <option value="yes" selected>Yes</option>
                <option value="no">No</option>
            </select>
        </div>-->

        <div class="top_add_space switch-toggle" title="partial messages will be sent, like in ChatGPT">
            <label>stream</label>
            <input id="select-stm" class="left_para for_focus" type="checkbox" checked name="stream">
            <label for="select-stm"></label>
        </div>

        <!-- select voice -->
        <div class="top_add_space" title="Select the timbre for the generated speech">
            <label>voice</label>
            <select id="select-voice" class="left_para for_focus" name="voice">
                <option disabled>--select speech voice--</option>
                <option value="1" selected>Alloy</option>
                <option value="2">Echo</option>
                <option value="3">Fable</option>
                <option value="4">Onyx</option>
                <option value="5">Nova</option>
                <option value="6">Shimmer</option>
            </select>
        </div>
    </div>

    <!-- chat part -->
    <div id="right-part" class="content">
        <!-- chat content region -->
        <div id="scrolldown" class="chat-content-area">
"###;
    let (logs_len, qa_num, logs) = get_log_for_display(uuid, true); // cookie对应的chat记录
    for (i, log) in logs.iter().enumerate() {
        if log.is_query { // 用户输入的问题
            result += &format!("            <!-- user -->
            <div class='right-time'>{}{}</div>
            <div class='user-chat-box'>
                <div class='q_icon_query'>
                    <div class='chat-txt right' id='{}' title='message {}, Q&A pair {}, {} tokens'></div>
                    <div class='chat-icon'>\n", if log.is_web {"🌐 "} else {""}, log.time, log.id, if logs_len > 0 {i+1} else {0}, log.idx_qa, log.token);
            if log.is_img || log.is_voice {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_USER);
            } else {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='click to copy' src='{}' />", log.id, ICON_USER);
            }
            result += r###"
                    </div>
                </div>
            </div>
"###;
        } else { // 答案
            result += &format!("            <!-- robot -->
            <div class='left-time'>{}</div>
            <div class='gpt-chat-box'>
                <div class='chat-icon'>\n", log.time);
            if log.is_img || log.is_voice {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_CHATGPT);
            } else {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='click to copy' src='{}' />", log.id, ICON_CHATGPT);
            }
            result += &format!("
                </div>
                <div class='chat-txt left' id='{}' title='message {}, Q&A pair {}, {} tokens'></div>
            </div>\n", log.id, if logs_len > 0 {i+1} else {0}, log.idx_qa, log.token);
        }
    }
    result += r###"        </div>

        <!-- user input region -->
        <div class="chat-inputs-container">
            <div class="chat-inputs-inner">
                <textarea autofocus name="Input your query" id="input_query" placeholder="Input your query"></textarea>
                <span id="submit_span" class="for_focus_button">
"###;
//                    <i class="search_btn fa fa-paper-plane" aria-hidden="true"></i>
    result += &format!("                    <img src='{}' class='search_btn' aria-hidden='true' />", ICON_SEND);
    result += r###"
                </span>
            </div>
        </div>

    </div>

    <!-- footer -->
    <footer>
        <button onclick='toggle()' id='left-toggle' title='Switch parameter bar settings'>
"###;
    result += &format!("            <img src='{}' aria-hidden='true' />", ICON_SETTING);
    result += r###"
        </button>
        <!-- <div>&copy; 2025 Copyright srx</div> -->
        <a href='https://github.com/jingangdidi'>https://github.com/jingangdidi</a>
    </footer>

    <script>
"###;
    result += &format!("{}\n", PRISM_MIN_JS);
    result += &format!("{}\n", MARKED_MIN_JS);
    result += r###"    </script>
    <script>
        // markdown转html
        function markhigh() {
"###;
    for (idx, log) in logs.iter().enumerate() {
        result += &format!("            var msg = document.getElementById('{}');
            var tmp = `{}`; // 这里将模板中的chat内容（已将“`”做了转译，“script”结束标签去掉了“<”）存入变量中
            if (tmp.startsWith('data:image/svg+xml;base64,')) {{ // 插入图片
                let tmp_img = document.createElement('img');
                tmp_img.src = tmp;
                msg.appendChild(tmp_img);\n", log.id, log.content);
        if log.is_voice {
            result += "                tmp_img.setAttribute('class', 'voice-size');\n"; // 设置voice图标大小
        }
        if !log.is_query { // 回答生成的图片或音频文件，添加hover下载按钮
            result += &format!("                let tmp_div = document.createElement('div');
                tmp_div.setAttribute('class', 'details');
                let tmp_a = document.createElement('a');
                tmp_a.setAttribute('class', 'title');
                tmp_a.setAttribute('href', 'http://{}:{}{}/save/{}');
                tmp_a.textContent = 'Download';
                tmp_div.appendChild(tmp_a);
                msg.setAttribute('class', 'chat-txt left tile'); // 加上tile
                msg.appendChild(tmp_div);\n", PARAS.addr_str, PARAS.port, v, idx);
        }
        result += r###"
            } else { // 文本问题或答案
                tmp = tmp.replaceAll('\\`', '`').replaceAll('/scrip', '</scrip'); // 恢复转译的“`”和“script”结束标签
"###;
        if log.is_query { // 用户输入的问题
            result += "                msg.textContent = tmp.replaceAll('\\\\n', '\\n');\n            }\n            // 问题不需要markdown解析\n";
        } else { // 答案
            result += "                msg.innerHTML = marked.parse(tmp).replaceAll('<p>', '').replaceAll('</p>', '');\n            }\n";
        }
    }
    result += &format!("        }}
        window.onload = markhigh();
        document.getElementById('show-prompt').value = '{}';
        document.getElementById('show-uuid').value = '{}';
        document.getElementById('show-in-token').value = '{}';
        document.getElementById('show-out-token').value = '{}';
", prompt_name, uuid, token[0], token[1]);
    result += "    </script>
</body>

<!-- js -->
<script type='text/javascript'>
";
    result += &format!("    var address = 'http://{}:{}{}/chat?q='; // http://127.0.0.1:8080\n    var current_id = {}; // 当前最新message的id，之后插入新问题或答案的id会基于该值继续增加\n    var qa_num = {}; // 问答对数量\n    var last_is_answer = true; // 最后一条信息是否是回答\n", PARAS.addr_str, PARAS.port, v, logs_len, qa_num);
    result += r###"    var emptyInput = true; // 全局变量，存储输入问题是否为空
    var no_message = true; // 是否没有获取到效回复，没有获取到，则将添加的msg_res删掉
    var already_clear_log = false; // 是否已清除了当前的记录
    var for_markdown = ''; // 累加原始信息，用于markdown显示
    // 左侧下拉菜单选取完成后，自动focus到问题输入框
    document.querySelectorAll('.for_focus').forEach(select => {
        select.addEventListener('change', function() {
            document.getElementById('input_query').focus();
        });
    });
    // 点击提交按钮和头像后，自动focus到问题输入框。由于头像消息是动态增加的，因此不能像上面那样，而应该使用事件委托
    document.addEventListener('click', function(event) {
        if (event.target.classList.contains('for_focus_button')) {
            document.getElementById('input_query').focus();
        }
    });
    // 停止接收回答
    let reader; // 接收答案
    let isStopped = true; // 是否停止接收答案
    // 左下按钮，切换左侧参数栏
    let toggleMain = true; // true显示主参数，false显示其余参数
    function toggle () {
        toggleMain = !toggleMain;
        const left_main = document.getElementById('left-part');
        const left_other = document.getElementById('left-part-other');
        if (toggleMain) {
            left_other.classList.add('animate');
            left_main.style.display = 'block';
            left_other.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_main.classList.remove('animate');
            });
        } else {
            left_main.classList.add('animate');
            left_other.style.display = 'block';
            left_main.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_other.classList.remove('animate');
            });
        }
    }
    // 上传文件，选好文件后直接提交，不需要submit按钮 https://stackoverflow.com/questions/7321855/how-do-i-auto-submit-an-upload-form-when-a-file-is-selected
    function sleep (time) {
        return new Promise((resolve) => setTimeout(resolve, time));
    }
    document.getElementById("upload-file").onchange = function(event) {
        document.getElementById("form").submit();
        for (let i = 0; i < event.target.files.length; i++) {
            const file = event.target.files[i];
            if (file) {
                insert_right_image(); // 先插入右侧的空内容，后面写入图片或上传文件的文件名
                let new_id = 'm'+(current_id-1);
                const msg_req_right = document.getElementById(new_id);
                if (file.type.startsWith('image/')) { // 插入显示上传的图片或文件名
                    // 生成临时URL并设置为图片的src
                    const objectURL = URL.createObjectURL(file);
                    let right_img = document.createElement("img");
                    right_img.src = objectURL;
                    msg_req_right.appendChild(right_img);
                } else { // 如果不是图片，显示上传文件的名称
                    msg_req_right.textContent = file.name;
                }
                sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                    scroll();
                });
            }
        }
        document.getElementById('input_query').focus();
        sleep(2000).then(() => {
            document.getElementById("form").reset();
        });
    };
    // 清空指定元素的所有子元素，https://stackoverflow.com/questions/3955229/remove-all-child-elements-of-a-dom-node-in-javascript
    function clear_all_child(id_name) {
        const parent = document.getElementById(id_name)
        while (parent.firstChild) {
            parent.firstChild.remove();
        }
    }
    // 更新相关uuid的下拉项
    function related_uuid(uuids) {
        clear_all_child('select-related-uuid');
        let options = document.getElementById("select-related-uuid");
        let disabled_option = document.createElement("option");
        disabled_option.setAttribute("value", "-1");
        disabled_option.setAttribute("disabled", "");
        disabled_option.setAttribute("selected", "");
        disabled_option.text = "--select uuid--";
        options.appendChild(disabled_option);
        for (i of uuids) {
            let uuid_option = document.createElement("option");
            uuid_option.setAttribute("value", i[0]);
            uuid_option.text = i[0]+' ('+i[1]+')';
            options.appendChild(uuid_option);
        }
    }
    // 插入右侧图片
    function insert_right_image() {
        /* 输入内容 */
        let msg_req_right = document.createElement("div");
        msg_req_right.setAttribute("class", "chat-txt right");
        let new_id = 'm'+current_id;
        current_id += 1; // id序号加1
        if (last_is_answer) {
            qa_num += 1;
            last_is_answer = false;
        }
        msg_req_right.setAttribute("title", "message "+current_id+", Q&A pair "+qa_num);
        msg_req_right.setAttribute("id", new_id);
        /* 头像 */
        let icon_div = document.createElement("div");
        icon_div.setAttribute("class", "chat-icon");
        let icon_right = document.createElement("img");
"###;
    result += &format!("        icon_right.setAttribute('src', '{}');\n", ICON_USER);
    result += r###"        icon_right.setAttribute("class", "chatgpt-icon for_focus_button");
        //icon_right.setAttribute("onclick", "copy('"+new_id+"');");
        //icon_right.setAttribute("title", "click to copy");
        icon_div.appendChild(icon_right);
        /* 提问的头像和内容放到一个div右侧对齐 */
        let q_icon_query_div = document.createElement("div");
        q_icon_query_div.setAttribute("class", "q_icon_query");
        q_icon_query_div.appendChild(msg_req_right);
        q_icon_query_div.appendChild(icon_div);
        /* 用户输入内容最外的div */
        let Con1 = document.createElement("div");
        Con1.setAttribute("class", "user-chat-box");
        /* chat区域插入输入内容和头像 */
        let message = document.getElementById("scrolldown");
        /* 提问的当前时间 */
        let timeInfo = document.createElement("div");
        timeInfo.setAttribute("class", "right-time");
        timeInfo.innerHTML = formatDate(true);
        message.appendChild(timeInfo);
        Con1.appendChild(q_icon_query_div);
        message.appendChild(Con1);
    }
    // 插入左侧答案和右侧问题
    function insert_left_right(message_content, message_time, id, is_left, is_img, is_voice, is_web, current_token) {
        if (id === current_id) { // 当前消息还没插入
            let new_id = 'm'+current_id; // 当前要插入消息的id
            current_id += 1; // id序号加1
            let msg_lr = document.createElement("div");
            msg_lr.setAttribute("id", new_id);
            if (is_img) { // 插入图片
                let lr_img = document.createElement("img");
                lr_img.src = message_content;
                msg_lr.appendChild(lr_img);
                if (is_left) { // 左侧图片hover时下载按钮
                    msg_lr.setAttribute("class", "chat-txt left tile"); // tile用于hover时下载图片或语音
                    let tmp_div = document.createElement('div');
                    tmp_div.setAttribute('class', 'details');
                    let tmp_a = document.createElement('a');
                    tmp_a.setAttribute('class', 'title');
                    if (is_voice) {
                        lr_img.setAttribute('class', 'voice-size'); // 设置voice图标大小
                    }
"###;
    result += &format!("                    tmp_a.setAttribute('href', 'http://{}:{}{}/save/'+(current_id-1));\n", PARAS.addr_str, PARAS.port, v);
    result += r###"                    tmp_a.textContent = 'Download';
                    tmp_div.appendChild(tmp_a);
                    msg_lr.appendChild(tmp_div);
                } else {
                    msg_lr.setAttribute("class", "chat-txt right");
                }
            } else {
                if (is_left) { // 文本答案
                    for_markdown = message_content.replaceAll('srxtzn', '\n');
                    msg_lr.setAttribute("class", "chat-txt left");
                    // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                    msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                    // 对每个代码块进行高亮
                    msg_lr.querySelectorAll('pre code').forEach((block) => {
                        Prism.highlightElement(block);
                    });
                } else { // 文本问题
                    msg_lr.setAttribute("class", "chat-txt right");
                    msg_lr.textContent = message_content.replaceAll('srxtzn', '\n').replaceAll('\\n', '\n'); // 不要使用innerHTML，innerHTML会识别标签将内容解析为html，textContent只是文本，innerText会受到css影响，https://stackoverflow.com/questions/31002593/type-new-line-character-in-element-textcontent
                }
            }
            /* 头像 */
            let icon_div = document.createElement("div");
            icon_div.setAttribute("class", "chat-icon");
            let icon_lr = document.createElement("img");
"###;
    result += &format!("            if (is_left) {{ // 答案
                icon_lr.setAttribute('src', '{}');
            }} else {{ // 问题
                icon_lr.setAttribute('src', '{}');
            }}
", ICON_CHATGPT, ICON_USER);
    result += r###"            icon_lr.setAttribute("class", "chatgpt-icon for_focus_button");
            if (!is_img) {
                icon_lr.setAttribute("onclick", "copy('"+new_id+"');");
                icon_lr.setAttribute("title", "click to copy");
            }
            icon_div.appendChild(icon_lr);

            /* 最外层提问/回答的当前时间 */
            let timeInfo = document.createElement("div");
            if (is_left) {
                timeInfo.setAttribute("class", "left-time");
            } else {
                timeInfo.setAttribute("class", "right-time");
            }
            if (is_web) {
                timeInfo.innerHTML = "🌐 "+message_time;
            } else {
                timeInfo.innerHTML = message_time;
            }

            /* chat区域插入问题/答案的时间 */
            let message = document.getElementById("scrolldown");
            message.appendChild(timeInfo);

            if (is_left) {
                last_is_answer = true;
                if (current_token > 0) {
                    msg_lr.setAttribute("title", "message "+current_id+", Q&A pair "+qa_num+", "+current_token+" tokens");
                } else {
                    msg_lr.setAttribute("title", "message "+current_id+", Q&A pair "+qa_num); // 这里先不显示token数，等回答完成后再加上
                }
                /* 答案外的div */
                let Con2 = document.createElement("div");
                Con2.setAttribute("class", "gpt-chat-box");
                /* chat区域插入答案和头像 */
                Con2.appendChild(icon_div);
                Con2.appendChild(msg_lr);
                /* 提问的当前时间 */
                message.appendChild(Con2);
            } else {
                if (last_is_answer) {
                    qa_num += 1;
                    last_is_answer = false;
                }
                if (current_token > 0) {
                    msg_lr.setAttribute("title", "message "+current_id+", Q&A pair "+qa_num+", "+current_token+" tokens");
                } else {
                    msg_lr.setAttribute("title", "message "+current_id+", Q&A pair "+qa_num); // 这里先不显示token数，等最后由MetaData加上
                }
                /* 提问的头像和内容放到一个div右侧对齐 */
                let q_icon_query_div = document.createElement("div");
                q_icon_query_div.setAttribute("class", "q_icon_query");
                q_icon_query_div.appendChild(msg_lr);
                q_icon_query_div.appendChild(icon_div);
                /* 用户输入内容最外的div */
                let Con1 = document.createElement("div");
                Con1.setAttribute("class", "user-chat-box");
                Con1.appendChild(q_icon_query_div);
                message.appendChild(Con1);
            }
        } else if (id === current_id - 1) { // 当前消息已经插入，继续追加内容。由于图片base64在一个stream中，因此这里只能是stream传输的文本答案
            let new_id = 'm'+id; // 当前要插入消息的id
            let msg_lr = document.getElementById(new_id);
            for_markdown += message_content.replaceAll('srxtzn', '\n');
            // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
            msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
            // 对每个代码块进行高亮
            msg_lr.querySelectorAll('pre code').forEach((block) => {
                Prism.highlightElement(block);
            });
        } else { // 不应该出现
            console.error("message id not match: current_id='${current_id}', received_id='${id}'");
        }
    }
    // 个位数左侧加0补为2位数，https://www.toptal.com/software/definitive-guide-to-datetime-manipulation
    function pad(n) {
        return n<10 ? '0'+n : n;
    }
    // 获取当前时间，并格式化为：2024-10-20 17:37:47，https://stackoverflow.com/questions/14638018/current-time-formatting-with-javascript
    function formatDate(is_user) {
        var d = new Date();
        var year = d.getFullYear();
        var month = pad(d.getMonth()+1); // 0-11
        var day = pad(d.getDate()); // 1-31
        var hr = pad(d.getHours()); // 0-23
        var min = pad(d.getMinutes()); // 0-59
        var sec = pad(d.getSeconds()); // 0-59
        if (is_user) {
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec;
        } else {
            // https://stackoverflow.com/questions/14976495/get-selected-option-text-with-javascript
            var sel = document.getElementById("select-model");
            var text= sel.options[sel.selectedIndex].text.split(" (")[0];
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec+" "+text;
        }
    }
    // 复制指定头像id对应的内容
    function copy(id) {
        // https://code-boxx.com/strip-remove-html-tags-javascript/
        var textToCopy = document.getElementById(id).textContent;
        //console.log(textToCopy);
        navigator.clipboard.writeText(textToCopy);
    }
    /* chat region scroll bottom */
    function scroll() {
        var scrollMsg = document.getElementById("scrolldown");
        scrollMsg.scrollTop = scrollMsg.scrollHeight;
    }
    // 获取用户发起提问时提交的信息
    function get_url() {
        var req = document.getElementById("input_query").value;
        if (req !== '') { // 输入不为空才不在界面显示输入内容
            emptyInput = false;
            // 插入用户输入内容
            //insert_left_right(req, formatDate(true), current_id, false, false, false); // 不在这里插入问题，后面问题会作为MainData插入，附带token数等信息
        } else {
            emptyInput = true;
        }
        // 清空输入框，滚动到最下面，等待答案
        document.getElementById("input_query").value = "";
        scroll();
        // https://stackoverflow.com/questions/1085801/get-selected-value-in-dropdown-list-using-javascript
        // 获取选择的模型
        var para_model = document.getElementById("select-model").value;
        // 获取选择的思考深度
        var para_effort = document.getElementById("select-effort").value;
        // 获取输入的对话名称
        var para_chat_name = document.getElementById("input-chat-name").value;
        // 获取输入的uuid
        var para_uuid = document.getElementById("input-uuid").value;
        if (para_uuid === '') { // 输入的uuid优先级要高于下拉选择的uuid
            para_uuid = document.getElementById("select-related-uuid").value;
            if (para_uuid === '-1') {
                para_uuid = '';
            }
        }
        // 获取输入的temperature
        var para_temperature = document.getElementById("input-temperature").value;
        // 获取选择的stream
        var para_stm = document.getElementById("select-stm").checked;
        // 获取是否网络搜索
        var para_web = document.getElementById("select-web").checked;
        // 获取选择的要保留的最近的最多问答记录数
        var para_num = document.getElementById("select-log-num").value;
        // 使用选择的prompt开启新对话
        var para_prompt = document.getElementById("select-prompt").value;
        // 使用选择生成音频的声音
        var para_voice = document.getElementById("select-voice").value;
        // 输入框无效，并显示信息
        if (emptyInput) { // 输入为空表示提问
            var q = 0;
            document.getElementsByName('Input your query')[0].placeholder = 'Waiting for answer ...';
        } else if (para_web) { // 使用网络搜索需要等待搜索结束
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = 'Waiting for search ...';
        } else { // 输入不为空表示用户继续提问
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = 'Sending query ...';
        }
        document.getElementById('input_query').disabled = true; // 完成回复之前禁止继续提问
        // 将参数加到问题后面
        req2 = q+"&model="+para_model+"&chatname="+para_chat_name+"&uuid="+para_uuid+"&stream="+para_stm+"&web="+para_web+"&num="+para_num+"&prompt="+para_prompt+"&voice="+para_voice+"&effort="+para_effort+"&temp="+para_temperature;
        return [req, req2];
    }
    // 提交问题并获取答案
    async function send_query_receive_answer() {
        // 从服务器获取stream内容
        no_message = true;
        already_clear_log = false;
        var autoScroll = true; // 默认随着流式输出自动滚动，如果用户进行了手动滚动，则停止自动滚动，这样就保持页面停留在用户想看的那个位置
        let tmpmsg = ""; // 累加存储流式输出的结果，转为markdown
        submit_send_stop = document.getElementById("submit_span");
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' style='width: 50px;' aria-hidden='true' />\";\n", ICON_STOP);
    result += r###"        isStopped = false;
        // 由于EventSource不支持post，因此无法将问题通过body传递，只能放到url中通过url参数传递，但url有长度限制（好像大部分浏览器是2k），因此输入内容长度不能太长
        // 这里用fetch发送post，将问题字符串通过body传递，其他简单参数通过url传递
        let [req, req2] = get_url();
        const response = await fetch(address+req2, {
            method: 'POST',
            headers: {
                'Content-Type': 'text/plain;charset=UTF-8',
                'Accept': 'text/event-stream'
            },
            body: req,
        });
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = ''; // Buffer to accumulate partial messages
        // 解析数据
        while (!isStopped) {
            const { done, value } = await reader.read();
            if (done) {
                // Process any remaining data in buffer if it forms a complete message
                if (buffer.trim()) processSseBuffer(); 
                break;
            }
            buffer += decoder.decode(value, { stream: true }); // stream: true is important
            processSseBuffer();
        }
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' aria-hidden='true' />\";\n", ICON_SEND);
    result += r###"        isStopped = true;
        document.getElementById("select-prompt").value = '-1'; // prompt恢复为不开启新会话
        document.getElementById("input-chat-name").value = ''; // 清空填写的对话名称
        document.getElementById("input-uuid").value = ''; // 清空填写的uuid，此时左下“current uuid”中显示的即是填写的uuid
        document.getElementById("input_query").value = "";
        document.getElementById('input_query').disabled = false; // 已完成回复，可以继续提问
        document.getElementsByName('Input your query')[0].placeholder = 'Input your query';
        document.getElementById("input_query").focus();

        // 解析完整数据
        function processSseBuffer() {
            let eolIndex;
            // SSE messages are separated by double newlines "\n\n"
            while ((eolIndex = buffer.indexOf('\n\n')) >= 0) {
                // 从buffer中获取“\n\n”之前的内容
                const messageStr = buffer.substring(0, eolIndex);
                // 从buffer中去除“\n\n”以及之前的内容，buffer此时剩下“\n\n”之后的内容
                buffer = buffer.substring(eolIndex + 2);
                // Skip empty messages
                if (messageStr.trim() === '') continue;
                // Parse the individual SSE message
                let currentEvent = 'maindata'; // Default event type
                let currentData = [];
                // 根据\n拆分解析每行，注意一个data内不要有\n，多行可以写到多个data中
                messageStr.split('\n').forEach(line => {
                    if (line.startsWith('event: ')) {
                        currentEvent = line.substring('event: '.length).trim();
                    } else if (line.startsWith('data: ')) {
                        currentData.push(line.substring('data: '.length));
                    } else {
                        console.warn("line not starts with event and data:", line);
                    }
                });
                // 用\n将data数据合并为一个字符串
                const eventData = currentData.join('\n');
                // 基于event类型解析数据
                try {
                    const jsonData = JSON.parse(eventData);
                    switch (currentEvent) {
                        case 'metadata':
                            if (jsonData.current_token > 0) { // 回答结束，更新token数
                                let answer_id = 'm'+(current_id - 1); // 当前回答的id
                                let msg_lr = document.getElementById(answer_id);
                                const currentTitle = msg_lr.getAttribute("title");
                                msg_lr.setAttribute("title", currentTitle + ", "+jsonData.current_token+" tokens");
                            }
                            //console.log('Received metadata:', jsonData);
                            // 更新页面左测当前uuid、问题token、答案token、prompt名称、相关uuid
                            document.getElementById("show-prompt").value = jsonData.prompt;
                            document.getElementById("show-uuid").value = jsonData.current_uuid;
                            document.getElementById("show-in-token").value = jsonData.in_token;
                            document.getElementById("show-out-token").value = jsonData.out_token;
                            related_uuid(jsonData.related_uuid);
                            if (autoScroll) {
                                scroll();
                            }
                            break; // 否则会继续执行下面的case
                        case 'maindata':
                            //console.log('Received maindata:', jsonData);
                            // 如果信息是之前的问答记录，先清空当前所有信息
                            if (!already_clear_log && jsonData.is_history) {
                                clear_all_child('scrolldown');
                                already_clear_log = true;
                                current_id = 0;
                                qa_num = 0;
                                last_is_answer = true;
                            }
                            // https://stackoverflow.com/questions/15275969/javascript-scroll-handler-not-firing
                            // https://www.answeroverflow.com/m/1302587682957824081
                            window.addEventListener('wheel', function() { // “scroll”无效
                                if (autoScroll) {
                                    //console.log('Scrolling via mouse');
                                    autoScroll = false; // 用户手动进行滚动，后面将不再自动滚动
                                }
                            });
                            window.addEventListener('touchmove', function() { // 触屏这个有效
                                if (autoScroll) {
                                    //console.log('Scrolling via touch');
                                    autoScroll = false; // 用户手动进行滚动，后面将不再自动滚动
                                }
                            });
                            no_message = false;
                            // 插入信息
                            if (jsonData.time_model) {
                                insert_left_right(jsonData.content, jsonData.time_model, jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                            } else { // 没有传递时间则使用当前时间
                                if (jsonData.is_left) {
                                    insert_left_right(jsonData.content, formatDate(false), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                                } else {
                                    insert_left_right(jsonData.content, formatDate(true), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token);
                                }
                            }
                            //Prism.highlightAll();
                            if (autoScroll) {
                                if (jsonData.is_img) {
                                    sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                                        scroll();
                                    });
                                } else {
                                    scroll();
                                }
                            }
                            break; // 否则会继续执行下面的case
                        case 'close':
                            //console.log('Received close:', jsonData);
                            break; // 否则会继续执行下面的case
                        default:
                            console.log(`Received unhandled event '${currentEvent}':`, jsonData);
                    }
                } catch (e) {
                    //console.error(`Failed to parse JSON for event '${currentEvent}':`, e, 'Raw data:', eventData);
                    console.log(`Failed to parse JSON for event '${currentEvent}':`, e, 'Raw data:', eventData);
                }
            }
        }
    }
    scroll();
    // 按下回车键发送
    document.getElementById("input_query").addEventListener("keypress", async(e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            if (isStopped) { // 发送问题
                await send_query_receive_answer();
            } else { // 停止接收回答
                if (reader) reader.cancel();
                isStopped = true;
            }
        }
    });
    // 鼠标点击按钮发送
    document.getElementById("submit_span").addEventListener("click", async(e) => {
        if (isStopped) { // 发送问题
            await send_query_receive_answer();
        } else { // 停止接收回答
            if (reader) reader.cancel();
            isStopped = true;
        }
    });
</script>

</html>
"###;
    result
}

/// 生成指定uuid对话记录的html字符串，css和js都写在html中，供下载使用
/// err_str不是None表示无法获取chat记录，记录的是错误信息
pub fn create_download_page(uuid: &str, err_str: Option<String>) -> String {
    // 创建包含css和js，并插入chat记录的html页面
    let mut result = r###"<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="author" content="srx">
    <title>chat log</title>
"###.to_string();
    //result += &format!("    <link rel='shortcut icon' href='{}/templates/images/robot-7.svg' type='image/x-icon'>\n", v);
    result += &format!("    <link rel='shortcut icon' href='{}' type='image/x-icon'>\n", ICON_SHORTCUT);
    result += "</head>\n";

    result += "<style type='text/css'>\n";
    result += CSS_CODE_DOWNLOAD;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += PRISM_MIN_CSS;
    result += "</style>\n";

    result += r###"<body>
    <div id="right-part" class="content">
        <!-- chat content region -->
        <div id="scrolldown" class="chat-content-area">
"###;
    // 获取该uuid的chat记录，如果传递的err_str不是None，则表示无法获取chat记录
    let logs = match err_str {
        Some(e) => vec![DisplayInfo{is_query: false, content:  e, id: "m0".to_string(), time: "".to_string(), is_img: false, is_voice: false, is_web: false, idx_qa: 1, token: 0}],
        None => {
            // 在保存当前chat记录之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
            pop_message_before_end(uuid);
            get_log_for_display(uuid, true).2 // cookie对应的chat记录
        },
    };
    for (i, log) in logs.iter().enumerate() {
        if log.is_query { // 用户输入的问题
            result += &format!("            <!-- user -->
            <div class='right-time'>{}{}</div>
            <div class='user-chat-box'>
                <div class='q_icon_query'>
                    <div class='chat-txt right' id='{}' title='message {}, Q&A pair {}, {} tokens'></div>
                    <div class='chat-icon'>\n", if log.is_web {"🌐 "} else {""}, log.time, log.id, i+1, log.idx_qa, log.token);
            if log.is_img || log.is_voice {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_USER);
            } else {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='click to copy' src='{}' />", log.id, ICON_USER);
            }
            result += r###"
                    </div>
                </div>
            </div>
"###;
        } else { // 答案
            result += &format!("            <!-- robot -->
            <div class='left-time'>{}</div>
            <div class='gpt-chat-box'>
                <div class='chat-icon'>\n", log.time);
            if log.is_img || log.is_voice {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_CHATGPT);
            } else {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' onclick=\"copy('{}');\" title='click to copy' src='{}' />", log.id, ICON_CHATGPT);
            }
            result += &format!("
                </div>
                <div class='chat-txt left' id='{}' title='message {}, Q&A pair {}, {} tokens'></div>
            </div>\n", log.id, i+1, log.idx_qa, log.token);
        }
    }
    result += r###"        </div>
    </div>

    <!-- footer -->
    <footer>
        <!--<div>https://github.com/jingangdidi</div>-->
        <a href='https://github.com/jingangdidi'>https://github.com/jingangdidi</a>
    </footer>

    <script>
"###;
    result += &format!("{}\n", PRISM_MIN_JS);
    result += &format!("{}\n", MARKED_MIN_JS);
    result += r###"    </script>
    <script>
        // markdown转html
        function markhigh() {
"###;
    for log in logs.iter() {
        result += &format!("            var msg = document.getElementById('{}');
            var tmp = `{}`; // 这里将模板中的chat内容（已将“`”做了转译，“script”结束标签去掉了“<”）存入变量中
            if (tmp.startsWith('data:image/svg+xml;base64,')) {{ // 插入图片
                let tmp_img = document.createElement('img');
                tmp_img.src = tmp;
                msg.appendChild(tmp_img);
            }} else {{ // 文本问题或答案
                tmp = tmp.replaceAll('\\`', '`').replaceAll('/scrip', '</scrip'); // 恢复转译的“`”和“script”结束标签\n", log.id, log.content);
        if log.is_query { // 用户输入的问题
            result += "                msg.textContent = tmp.replaceAll('\\\\n', '\\n');\n            }\n // 问题不需要markdown解析\n";
        } else { // 答案
            result += "                msg.innerHTML = marked.parse(tmp).replaceAll('<p>', '').replaceAll('</p>', '');\n            }\n";
        }
    }
    result += r###"        }
        window.onload = markhigh();
        function copy(id) {{
            // https://code-boxx.com/strip-remove-html-tags-javascript/
            var textToCopy = document.getElementById(id).textContent;
            navigator.clipboard.writeText(textToCopy);
        }}
    </script>
</body>
</html>
"###;
    result
}
