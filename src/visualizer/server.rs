use actix::{Actor, StreamHandler, ActorContext};
use actix_files::Files;
use actix_web::{middleware::Logger, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

struct WebSocket;

impl Actor for WebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocket {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                // echo the text back
                ctx.text(format!("Echo: {}", text));
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

// route handler
async fn ws_index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(WebSocket {}, &req, stream)
}

pub async fn start() -> anyhow::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .route("/ws/", web::get().to(ws_index))
            .service(Files::new("/", "static/").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await?;

    Ok(())
}
