//! Live integration tests — hit the real `jiosaavn.com`.
//!
//! Run with: `cargo test -- --ignored`
//! Test vectors are the ones used in the reference repo's own specs.

use saavnapi::{ArtistArgs, SaavnClient, SortBy, SortOrder};

fn client() -> SaavnClient {
    SaavnClient::new()
}

#[tokio::test]
#[ignore]
async fn song_by_id() {
    let songs = client().get_songs_by_ids(&["3IoDK8qI"]).await.unwrap();
    assert_eq!(songs[0].name, "Levitating");
    assert_eq!(songs[0].download_url.len(), 5);
    assert!(songs[0].download_url[4].url.ends_with("_320.mp4"));
}

#[tokio::test]
#[ignore]
async fn song_by_link() {
    let songs = client()
        .get_songs_by_link("https://www.jiosaavn.com/song/houdini/OgwhbhtDRwM")
        .await
        .unwrap();
    assert_eq!(songs[0].name, "Houdini");
}

#[tokio::test]
#[ignore]
async fn album_by_id() {
    let album = client().get_album_by_id("23241654").await.unwrap();
    assert_eq!(album.name, "Future Nostalgia");
    assert!(album.songs.map(|s| !s.is_empty()).unwrap_or(false));
}

#[tokio::test]
#[ignore]
async fn search_songs_and_all() {
    let results = client().search_songs("levitating", 0, 2).await.unwrap();
    assert!(results.total > 0);
    assert!(!results.results.is_empty());
    let all = client().search_all("levi").await.unwrap();
    assert!(!all.songs.results.is_empty());
}

#[tokio::test]
#[ignore]
async fn artist_by_id() {
    let artist = client()
        .get_artist_by_id("1274170", ArtistArgs::default())
        .await
        .unwrap();
    assert_eq!(artist.name, "Dua Lipa");
}

#[tokio::test]
#[ignore]
async fn playlist_by_id_and_link() {
    let by_id = client().get_playlist_by_id("82914609", 2, 0).await.unwrap();
    assert_eq!(by_id.name, "Best of Indie - English");
    let by_link = client()
        .get_playlist_by_link(
            "https://www.jiosaavn.com/featured/its-indie-english/AMoxtXyKHoU_",
            2,
            0,
        )
        .await
        .unwrap();
    assert_eq!(by_link.id, by_id.id);
}

#[tokio::test]
#[ignore]
async fn launch_data() {
    let data = client().get_launch_data(None).await.unwrap();
    assert!(!data.new_trending.is_empty());
    assert!(!data.new_albums.is_empty());
    assert!(!data.top_playlists.is_empty());
    assert!(!data.charts.is_empty());
    assert!(!data.modules.is_empty());
    // The new_trending module's view_more points at content.getTrending.
    let module = data
        .modules
        .get("new_trending")
        .expect("new_trending module present");
    assert_eq!(
        module.view_more.as_ref().unwrap().api,
        "content.getTrending"
    );
    // Modules whose view_more is `[]` on the wire must parse as None.
    let radio = data.modules.get("radio").expect("radio module present");
    assert_eq!(
        radio.view_more.as_ref().unwrap().api,
        "webradio.getFeaturedStations"
    );
    let promos = data
        .modules
        .get("promo:vx:data:68")
        .expect("promo module present");
    assert!(promos.view_more.is_none());
}

#[tokio::test]
#[ignore]
async fn launch_data_hindi_has_city_mod() {
    // city_mod only appears with a language (hindi/tamil/bengali;
    // english and punjabi did not return it, 2026-11-25).
    let data = client().get_launch_data(Some("hindi")).await.unwrap();
    assert!(!data.city_mod.is_empty());
    assert!(data.modules.contains_key("city_mod"));
    assert!(!data.new_trending.is_empty());
}

#[tokio::test]
#[ignore]
async fn trending_and_browse_modules() {
    let trending = client().get_trending(0, 5).await.unwrap();
    assert_eq!(trending.len(), 5);
    let modules = client().get_browse_modules().await.unwrap();
    assert!(modules.contains_key("new_trending"));
    assert!(modules.contains_key("top_playlists"));
}

#[tokio::test]
#[ignore]
async fn lyrics_present_and_absent() {
    // "Rula Ke Gaya Ishq" (bPBAx5IK) has lyrics live (2026-11-25).
    let lyrics = client().get_lyrics("bPBAx5IK").await.unwrap();
    let lyrics = lyrics.expect("song has lyrics live");
    assert!(lyrics.text.contains("<br>"));
    // "Levitating" has none — server says failure, client maps to None.
    assert!(client().get_lyrics("3IoDK8qI").await.unwrap().is_none());
}

#[tokio::test]
#[ignore]
async fn reco_song_album_playlist() {
    // reco.getreco needs android ctx — the client handles that.
    let recos = client().get_song_recommendations("3IoDK8qI").await.unwrap();
    assert!(!recos.is_empty());
    let albums = client()
        .get_album_recommendations("23241654")
        .await
        .unwrap();
    assert!(!albums.is_empty());
    let playlists = client()
        .get_playlist_recommendations("82914609")
        .await
        .unwrap();
    assert!(!playlists.is_empty());
}

#[tokio::test]
#[ignore]
async fn discovery_lists() {
    let charts = client().get_charts().await.unwrap();
    assert_eq!(charts.len(), 9);
    let albums = client().get_albums(0, 5, None).await.unwrap();
    assert_eq!(albums.items.len(), 5);
    assert!(albums.count > 0);
    let featured = client()
        .get_featured_playlists(0, 3, Some("hindi"))
        .await
        .unwrap();
    assert_eq!(featured.items.len(), 3);
    let shows = client().get_top_shows(0, 5).await.unwrap();
    assert_eq!(shows.shows.len(), 5);
    assert!(!shows.trending_podcasts.is_empty());
    assert!(shows.trending_title.is_some());
    let artists = client().get_top_artists().await.unwrap();
    assert_eq!(artists.len(), 30);
    assert_eq!(artists[0].name, "Arijit Singh");
    let searches = client().get_top_searches(None).await.unwrap();
    assert_eq!(searches.len(), 10);
    let stations = client().get_featured_stations(None).await.unwrap();
    assert_eq!(stations.len(), 28);
    let top_albums = client()
        .get_top_albums_of_year(2024, Some("hindi"))
        .await
        .unwrap();
    assert_eq!(top_albums.len(), 24);
}

#[tokio::test]
#[ignore]
async fn shows_and_episodes() {
    let show = client()
        .get_show_by_link("https://www.jiosaavn.com/shows/shri-krishna-amritvani/1/q3nfP8Sr8ZM_")
        .await
        .unwrap();
    assert_eq!(show.title, "Shri Krishna Amritvani");
    assert!(!show.seasons.is_empty());
    assert!(!show.episodes.is_empty());
    let episodes = client()
        .get_show_episodes(&show.id, Some(1), 1)
        .await
        .unwrap();
    assert!(!episodes.is_empty());
    assert_eq!(episodes[0].show_id.as_deref(), Some(show.id.as_str()));
}

#[tokio::test]
#[ignore]
async fn radio_creation_and_auth_token() {
    let station = client()
        .create_featured_station("Arijit Singh Radio", Some("hindi"))
        .await
        .unwrap();
    assert!(!station.is_empty());
    let artist_station = client()
        .create_artist_station("Arijit Singh", "Arijit Singh", Some("hindi"))
        .await
        .unwrap();
    assert!(!artist_station.is_empty());
    // Signed URL for "Levitating" at 96kbps.
    let songs = client().get_songs_by_ids(&["3IoDK8qI"]).await.unwrap();
    let encrypted = songs[0]
        .encrypted_media_url
        .as_deref()
        .expect("encrypted_media_url present");
    let token = client().generate_auth_token(encrypted, 96).await.unwrap();
    assert!(token.auth_url.starts_with("https://web.saavncdn.com/"));
    assert!(token.auth_url.contains("_96.mp4"));
}

#[tokio::test]
#[ignore]
async fn chrome_and_artist_top_songs() {
    let footer = client().get_footer_details("hindi").await.unwrap();
    assert!(!footer.playlists.is_empty());
    assert!(!footer.artists.is_empty());
    let menu = client().get_browse_hover_details().await.unwrap();
    assert_eq!(menu.top_artists.len(), 10);
    assert_eq!(menu.top_playlists.len(), 10);
    // Arijit Singh is populated live (Dua Lipa returns [] — see inventory).
    let songs = client()
        .get_artist_top_songs(
            "459320",
            "bPBAx5IK",
            0,
            Some(SortBy::Popularity),
            Some(SortOrder::Desc),
            Some("hindi"),
        )
        .await
        .unwrap();
    assert!(!songs.is_empty());
}

#[tokio::test]
#[ignore]
async fn song_suggestions() {
    // Station creation works; `webradio.getSong` itself currently
    // answers "No new song found for current radio." (verified live
    // 2026-11-25, 4 attempts, 2 songs). Accept either a real list or
    // that server-side error — the test guards the client flow, not
    // Saavn's radio backend.
    let station = client().create_song_station("3IoDK8qI").await.unwrap();
    assert!(!station.is_empty());
    match client().get_song_suggestions("3IoDK8qI", 5).await {
        Ok(suggestions) => {
            assert!(!suggestions.is_empty());
            assert!(suggestions.len() <= 5);
        }
        Err(e) => assert!(
            e.to_string().contains("No new song found"),
            "unexpected suggestions error: {e}"
        ),
    }
}
