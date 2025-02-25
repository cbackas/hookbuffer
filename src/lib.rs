use structs::sonarr::SonarrRequestBody;
use worker::*;

mod queue;
mod send;
mod structs;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .on_async(
            "/api/webhooks/:id/:token",
            async |req: Request, ctx: RouteContext<()>| {
                match req.headers().get("User-Agent") {
                    Ok(Some(user_agent)) if user_agent.starts_with("Sonarr/") => {}
                    _ => return Response::error("Invalid User-Agent", 400),
                };
                let group_id = ctx.param("id").unwrap();

                console_log!("Recieved webhook for group: {}", group_id);

                let namespace = ctx.durable_object("HOOKBUFFER")?;
                let stub = namespace.id_from_name(group_id)?.get_stub()?;
                stub.fetch_with_request(req).await
            },
        )
        .run(req, env)
        .await
}
