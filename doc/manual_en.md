# User Guide & Feature Overview

## Initiate a New Conversation
- On the left panel, `start new chat` displays prompts defined in `config.txt`. Selecting any option triggers a fresh conversation. The name of the active prompt appears under "current prompt" in the sidebar.
- The `maxage` setting in `config.txt` determines how long a conversation remains persistent after its last interaction. If the time elapsed exceeds this threshold, such as the default 1DAY, a new session will automatically begin upon re-entry. Previous conversations may still be accessed by entering their `uuid`. For instance, if the final query was made at 9 AM, the same session persists until 9 AM the next day, even without closing the page; beyond that window, a new chat is initiated. This mechanism prevents unrelated queries from polluting a single dialogue thread.
- Each query allows you to define or modify the current conversation title.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/start_new_chat_en.png" width="20%">
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/prompt_1.5x.gif">

## Continuous Queries and Responses
- After submitting a question, no immediate request is sent to the LLM. Instead, pressing Enter again while the input field is empty confirms and transmits the message. This design enables users to enter multiple questions sequentially, for example, breaking down a complex query into parts over several inputs.
- Should you remain dissatisfied with the model's response, simply press Enter (after optionally switching models via the `model` dropdown on the left). This discards all subsequent replies following the last input, prompting a fresh generation.
- When a tool is selected in the `call tools` section on the left, each invocation returns as an individual message block, enabling the display of multiple answer segments simultaneously.
<img src="https://github.com/jingangdidi/chatsong/raw/main/assets/image/QA-pair.png" width="50%">
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/multiple_models_1.5x.gif">

## Context Length Management
- This dropdown allows customization of how many prior chat logs are included when sending a new query to the model.
- Each message corresponds to either one user query or one model response.
- A Q&A pair represents a sequence of consecutive inputs followed by corresponding outputs, this does not limit total message count but rather enforces a round-based constraint. For example: a user uploads an image, poses a follow-up question, receives an answer from Model A, then switches to Model B for a revised response, all four messages (two inputs, two outputs) constitute a single conversational round.
- The option labeled `prompt + xxx` appends the initial prompt selected at session start as the first message in every request. If `no prompt` was chosen during initiation, no additional prompt is injected.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/select_context_length_en.png" width="20%">

## Tool Invocation
- By default, the system operates in standard Q&A mode without tool usage. Enabling tools via the `call tools` panel on the left activates external capabilities. Each tool execution yields a separate message, displayed individually until the model ceases further calls.
- For intricate tasks, activate `plan mode` (available only when `call tools` are enabled). This prompts the model to first decompose the original query into subtasks, resolving them sequentially. Each step generates a distinct message, ensuring transparency. If neither the model nor the selected tools can fulfill the request, the system halts and returns an explanation detailing missing capabilities.
- Tools may be selected individually or in groups. The more tools engaged, the higher the token consumption.
- Built-in support includes the `file system` tool, offering functionalities such as file reading/writing, compression, and extraction.
- Custom MCP stdio tools, such as Excel manipulation or network access, can be configured in `config.txt`, following the syntax detailed in [config_template.txt](https://github.com/jingangdidi/chatsong/blob/main/config_template.txt).
- External scripts (e.g., Python script) may also be integrated via `config.txt`, provided arguments are prefixed with `--`. Refer to [config_template.txt](https://github.com/jingangdidi/chatsong/blob/main/config_template.txt) for precise formatting guidelines.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/call_tools_en.png" width="20%">
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/mcp_1.5x.gif">

## Context Compression & Summarization
- When a conversation accumulates extensive history, transmitting all prior messages with each query becomes inefficient due to high token cost. If earlier interactions are irrelevant to the current inquiry, use `contextual messages` to restrict the number of included messages.
- When the dialogue is both lengthy and contextually cohesive, yet efficiency is desired, click the `Compress & Summarize` button (fourth from the left in the bottom-left corner). This condenses the most recent Q&A pairs within the specified context range into a single, concise summary message, reducing both token usage and context load.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/left_bottom_button.png" width="50%">

## Token Usage Statistics
- The three entries at the far left of the interface display cumulative token counts: total tokens sent to the LLM, total received from it, and the combined volume of the most recent request-response cycle.
- Additionally, hovering over any response message reveals the exact token count associated with that reply.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/token_en.png" width="20%">

## Upload Attachments for Inquiry
- Click the attachment icon beside the input field to upload images, text files, audio files, PDFs, or ZIP archives.
- Supported image formats: `.png`, `.jpg`, `.jpeg`.
- Audio formats accepted: `.flac`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.m4a`, `.ogg`, `.wav`, `.webm`.
- For PDFs with lowercase `.pdf` extension, each page is converted into a visual image (approximately 1000 tokens per page), enabling image-based reasoning with models like `qwen3-vl`. Otherwise, text content is extracted directly for processing.
- ZIP archives are recursively unpacked, merging all contents into a unified codebase while preserving original directory structure, ideal for querying project repositories.
- Any other file format defaults to plain text interpretation: its entire content is read and submitted as input.

## Image-Based Question Answering
- Requires a vision-capable model, such as the `qwen3-vl` series.
- Use the attachment button to upload images or PDFs.
- When uploading a pdf file (with lowercase extension), the system automatically converts each page into a standalone image (approx. 1000 tokens/page), which appear as discrete message blocks on the right side. Before finalizing the query by pressing Enter, you may manually delete irrelevant pages, such as reference sections, using the delete icon above each image, thereby conserving tokens.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/upload_pdf_to_image.png" width="20%">

## Download Current Conversation Record
- Every conversation is saved as a JSON file under the designated output path, uniquely identified by its uuid.
- To export the current session, click the `save` button (second from the left in the bottom-left corner), generating a self-contained, dependency-free HTML file.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/left_bottom_button.png" width="50%">

## Incognito Mode
- To prevent local storage of the current conversation, activate `Incognito Mode` via the button on the bottom-left (first from the right).
- A black eye icon indicates that incognito mode is enabled. Upon closing the browser tab, reopening it, or refreshing, the conversation history vanishes. Furthermore, terminating the chatsong service will discard the session without saving, ensuring complete privacy.
<img src="https://github.com/jingangdidi/chatsong/raw/main/doc/left_bottom_button.png" width="50%">

## Internal Network Multi-User Access
- On machine A within your local network, launch `chatsong` using the command-line flag `-a <ip>`, e.g., `-a 192.168.1.5`, to bind the service to a specific IP address. Other users on the same network can then access the API at `http://192.168.1.5:8080/v1`.
- `chatsong` treats all conversations uniformly, storing each under a unique uuid in machine A's output directory, regardless of user origin.
- To share a conversation, simply provide the uuid. Others can retrieve it by entering the uuid in the UUID Input box (accessible via the settings icon at the bottom-left corner), then typing any placeholder text and pressing Enter, this instantly navigates them to the shared session.
