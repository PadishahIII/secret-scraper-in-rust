use secret_scraper::logging::init_tracing;

fn main() {
    let _guard = init_tracing();
    let a = 1;
    let b = "b";
    #[derive(Debug, Copy, Clone)]
    struct Position {
        x: u32,
        y: u32,
    }
    let po = Position { x: 1, y: 2 };
    tracing::info!(a, b = b, position=?po,"test: {:?}", po);
}
