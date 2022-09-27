#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

//! comics is a simple comics server

use std::{
    fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::Parser;
use log::{debug, error, info};
use warp::{hyper::StatusCode, Filter};

#[derive(Parser)]
#[clap(about, author, version)]
struct Opts {
    /// Bind host and port
    #[clap(short, long, default_value = "127.0.0.1:3000")]
    bind: String,
    /// Data directory
    #[clap(short, long, default_value = "./data")]
    data_dir: String,
}

#[derive(Debug)]
struct Comic {
    cover: PathBuf,
    name: String,
    path: PathBuf,
}

fn list_comics<T>(data_dir: T) -> io::Result<Vec<Comic>>
where
    T: AsRef<Path>,
{
    let data_dir = data_dir.as_ref();

    let mut comics = vec![];

    for entry in fs::read_dir(data_dir)? {
        let dir = entry?;
        let metadata = dir.metadata()?;
        if metadata.is_dir() {
            let mut pages = vec![];
            for file in fs::read_dir(dir.path())? {
                let file = file?;
                let metadata = file.metadata()?;
                if metadata.is_file() && !metadata.is_symlink() {
                    pages.push(file.path().to_path_buf());
                }
            }

            pages.sort_by(|a, b| {
                a.to_string_lossy()
                    .partial_cmp(&b.to_string_lossy())
                    .unwrap()
            });

            if let Some(cover) = pages.first() {
                let name = dir.path();
                let name = name
                    .file_name()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or("untitled".into());
                debug!("load comic {name}");
                let comic = Comic {
                    cover: cover.to_path_buf(),
                    name: name.into(),
                    path: dir.path().to_path_buf(),
                };
                comics.push(comic);
            }
        }
    }

    comics.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());

    let count = comics.len();
    info!("{count} comic(s) loaded");

    Ok(comics)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opts = Arc::new(Opts::parse());

    let opts_c = opts.clone();
    let comics = Arc::new(Mutex::new(list_comics(&opts_c.data_dir)?));
    let comics_m = warp::any().map(move || comics.clone());

    let opts_c = opts.clone();
    let opts_m = warp::any().map(move || opts_c.clone());

    let index_route =
        warp::path::end()
            .and(comics_m.clone())
            .map(|comics: Arc<Mutex<Vec<Comic>>>| {
                let _comics = comics.lock().unwrap();
                format!("root")
            });

    let refresh_route = warp::path("refresh")
        .and(opts_m.clone())
        .and(comics_m.clone())
        .map(|opts: Arc<Opts>, comics: Arc<Mutex<Vec<Comic>>>| {
            let mut comics = comics.lock().unwrap();
            let new_comics = match list_comics(&opts.data_dir) {
                Err(e) => {
                    error!("{e:?}");
                    return format!("cannot refresh");
                }
                Ok(cs) => cs,
            };
            *comics = new_comics;
            format!("refresh")
        });

    let comic_route = warp::path!("comic" / String).and(comics_m.clone()).map(
        |path: String, comics: Arc<Mutex<Vec<Comic>>>| {
            let comics = comics.lock().unwrap();
            let (s, r) = if let Some(comic) = comics.iter().find(|c| c.name == path.as_str()) {
                (StatusCode::OK, warp::reply::html(format!("{comic:?}")))
            } else {
                (StatusCode::NOT_FOUND, warp::reply::html("".into()))
            };
            warp::reply::with_status(r, s)
        },
    );

    let data_dir = opts.data_dir.clone();
    let static_route = warp::path("static").and(warp::fs::dir(data_dir));

    let router = index_route
        .or(refresh_route)
        .or(comic_route)
        .or(static_route);

    let bind: SocketAddr = opts.bind.parse()?;
    warp::serve(router).run(bind).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_list_comics() {
        let comics = list_comics("./data").unwrap();
        assert_eq!(3, comics.len());

        let comic = comics.get(0).unwrap();
        assert_eq!(
            "./data/comic+01/001.png",
            comic.cover.to_string_lossy().to_string()
        );

        let comic = comics.get(1).unwrap();
        assert_eq!(
            "./data/comic01/001.png",
            comic.cover.to_string_lossy().to_string()
        );

        let comic = comics.get(2).unwrap();
        assert_eq!(
            "./data/comic02/002.png",
            comic.cover.to_string_lossy().to_string()
        );
    }
}
