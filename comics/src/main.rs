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
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use askama::Template;
use clap::Parser;
use log::{debug, error, info};
use pathdiff::diff_paths;
use warp::{
    hyper::{StatusCode, Uri},
    Filter,
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    comics: &'a Vec<Comic>,
    updated: String,
}

#[derive(Template)]
#[template(path = "comic.html")]
struct ComicTemplate<'a> {
    comic: &'a Comic,
}

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

mod filters {
    pub fn urlencode(s: &str) -> askama::Result<String> {
        Ok(urlencoding::encode(s).into())
    }
}

#[derive(Debug)]
struct Page {
    name: String,
}

#[derive(Debug)]
struct Comic {
    cover: PathBuf,
    name: String,
    pages: Vec<Page>,
}

#[derive(Debug)]
struct Comics {
    comics: Vec<Comic>,
    updated: chrono::DateTime<chrono::Local>,
}

fn list_comics<T>(data_dir: T) -> io::Result<Comics>
where
    T: AsRef<Path>,
{
    let data_dir = data_dir.as_ref();

    let mut comics = vec![];

    for entry in fs::read_dir(data_dir)? {
        let dir = entry?;
        let metadata = dir.metadata()?;

        if !metadata.is_dir() {
            continue;
        }

        let mut pages = vec![];
        for file in fs::read_dir(dir.path())? {
            let file = file?;
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            if metadata.is_symlink() {
                continue;
            }
            let path = match diff_paths(file.path().to_path_buf(), data_dir) {
                Some(p) => p,
                None => continue,
            };
            pages.push(path);
        }

        pages.sort_by(|a, b| {
            a.to_string_lossy()
                .partial_cmp(&b.to_string_lossy())
                .unwrap()
        });

        let cover = match pages.first() {
            Some(c) => c,
            None => continue,
        };

        let name = dir.path();
        let name = match name.file_name() {
            Some(s) => s.to_string_lossy(),
            None => continue,
        };

        debug!("load comic {name}");

        let pages = pages
            .iter()
            .map(|p| Page {
                name: p.to_string_lossy().to_string(),
            })
            .collect::<Vec<Page>>();

        let comic = Comic {
            cover: cover.to_path_buf(),
            name: name.into(),
            pages,
        };
        comics.push(comic);
    }

    comics.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());

    let count = comics.len();
    info!("{count} comic(s) loaded");

    let comics = Comics {
        updated: chrono::Local::now(),
        comics,
    };
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

    let index_route = warp::path::end()
        .and(comics_m.clone())
        .map(|comics: Arc<Mutex<Comics>>| {
            let comics = comics.lock().unwrap();
            let comics = comics.deref();
            let tpl = IndexTemplate {
                comics: &comics.comics,
                updated: comics.updated.to_rfc3339(),
            };
            let html = match tpl.render() {
                Ok(s) => s,
                Err(e) => {
                    error!("{e}");
                    format!("failed to render template").into()
                }
            };
            warp::reply::html(html)
        });

    let refresh_route = warp::path("refresh")
        .and(opts_m.clone())
        .and(comics_m.clone())
        .map(|opts: Arc<Opts>, comics: Arc<Mutex<Comics>>| {
            let mut comics = comics.lock().unwrap();
            let new_comics = match list_comics(&opts.data_dir) {
                Err(e) => {
                    error!("{e}");
                    return warp::redirect(Uri::from_static("/"));
                }
                Ok(cs) => cs,
            };
            *comics = new_comics;
            warp::redirect(Uri::from_static("/"))
        });

    let comic_route = warp::path!("comic" / String).and(comics_m.clone()).map(
        |path: String, comics: Arc<Mutex<Comics>>| {
            let comics = comics.lock().unwrap();
            let path = match urlencoding::decode(path.as_str()) {
                Err(e) => {
                    error!("{e}");
                    return warp::reply::with_status(
                        warp::reply::html("".into()),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }
                Ok(p) => p,
            };
            let comic = match comics.comics.iter().find(|c| c.name == path) {
                Some(comic) => comic,
                None => {
                    return warp::reply::with_status(
                        warp::reply::html("not found".into()),
                        StatusCode::NOT_FOUND,
                    )
                }
            };
            let tpl = ComicTemplate { comic };
            match tpl.render() {
                Ok(s) => warp::reply::with_status(warp::reply::html(s), StatusCode::OK),
                Err(e) => {
                    error!("{e}");
                    return warp::reply::with_status(
                        warp::reply::html("".into()),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }
            }
        },
    );

    let data_dir = opts.data_dir.clone();
    let static_route = warp::path("static").and(warp::fs::dir(data_dir));

    let log = warp::log("comics::server");
    let router = index_route
        .or(comic_route)
        .or(static_route)
        .or(refresh_route)
        .with(log);

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

        let comics = comics.comics;
        assert_eq!(3, comics.len());

        let comic = comics.get(0).unwrap();
        assert_eq!(
            "comic+01/001.png",
            comic.cover.to_string_lossy().to_string()
        );

        let comic = comics.get(1).unwrap();
        assert_eq!("comic01/001.png", comic.cover.to_string_lossy().to_string());

        let comic = comics.get(2).unwrap();
        assert_eq!("comic02/002.png", comic.cover.to_string_lossy().to_string());
    }
}
