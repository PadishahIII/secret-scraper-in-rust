use secret_scraper::logging::{LogLevel, init_tracing_with_level};

fn main() {
    let _guard = init_tracing_with_level(LogLevel::Info);
    let a = 1;
    let b = "b";
    #[derive(Debug, Copy, Clone)]
    struct Position {
        x: u32,
        y: u32,
    }
    let po = Position { x: 1, y: 2 };
    let _coordinate_sum = po.x + po.y;
    tracing::info!(a, b = b, position=?po,"test: {:?}", po);
}
