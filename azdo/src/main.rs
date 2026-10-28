use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let state = azdo::AppState::from_env();
    let port = std::env::var("AZDO_PORT")
        .or_else(|_| std::env::var("AZURZ_AZDO_PORT"))
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .unwrap_or_else(|error| panic!("écoute impossible sur {address}: {error}"));
    println!("Azure DevOps pour Linux sur http://{address}");
    axum::serve(listener, azdo::router(state))
        .await
        .unwrap_or_else(|error| panic!("serveur arrêté: {error}"));
}
