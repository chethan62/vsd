use vsd::{FileDownloader, Result, reqwest::Client, tokio};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let client = Client::new();

    FileDownloader::new(&client)
        .threads(5)
        .resume(true)
        .download(
            "https://github.com/llvm/llvm-project/releases/download/llvmorg-22.1.6/llvm-project-22.1.6.src.tar.xz",
            "target/llvm-project-22.1.6.src.tar.xz",
        )
        .await?;

    println!("Download complete!");
    Ok(())
}
