pub mod hello; // `GET /嵌套的前缀/hello`
pub mod demo_status; // `GET /嵌套的前缀/demo-status`
pub mod user; // `POST /嵌套的前缀/create-user`和`GET /嵌套的前缀/users`
pub mod demo_uri; // `GET /嵌套的前缀/get-uri`
pub mod multi_foo; // `GET,PUT,PATCH,POST,DELET /嵌套的前缀/multi-foo`
pub mod get_items_id; // `GET /嵌套的前缀/items/:id`
pub mod get_items; // `GET /嵌套的前缀/items`
pub mod demo_json; // `PUT /嵌套的前缀/demo-json`和`GET /嵌套的前缀/demo-json`
pub mod demo_csv; // `GET /嵌套的前缀/demo-csv`
pub mod index; // `GET /嵌套的前缀`
pub mod chat; // `GET /嵌套的前缀/chat`
pub mod save; // `GET /嵌套的前缀/save`
pub mod delete_message; // `GET /嵌套的前缀/delmsg/:id`
pub mod incognito; // `GET /嵌套的前缀/incognito`
pub mod upload; // `POST /嵌套的前缀/upload`
pub mod usage; // `GET /嵌套的前缀/usage`
pub mod fallback; // `NOT_FOUND`
