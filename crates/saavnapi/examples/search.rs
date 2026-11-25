//! `cargo run --example search -- levitating`
//! Searches songs, then fetches the first hit's full record.

use saavnapi::SaavnClient;

#[tokio::main]
async fn main() -> Result<(), saavnapi::Error> {
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "levitating".to_string());
    let client = SaavnClient::new();

    let results = client.search_songs(&query, 0, 5).await?;
    println!(
        "{} results (total {})",
        results.results.len(),
        results.total
    );
    for song in &results.results {
        println!("- {} — {} ({:?})", song.name, song.id, song.year);
    }

    if let Some(first) = results.results.first() {
        let full = client.get_songs_by_ids(&[&first.id]).await?;
        let song = &full[0];
        println!("\n{} — {}", song.name, song.url);
        for link in &song.download_url {
            println!("  {}: {}", link.quality, link.url);
        }
    }
    Ok(())
}
