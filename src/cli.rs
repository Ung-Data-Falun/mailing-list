use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,
}
