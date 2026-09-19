fn main() {
    for s in ["1h52m3s", "0s", "500ms", "00:00:02,500"] {
        println!("{s} -> {:?}", jianying_cli::tim::parse(s));
    }
}
