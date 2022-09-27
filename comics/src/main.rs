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
};

use clap::Parser;
use warp::Filter;

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

            if let Some(page) = pages.first() {
                let comic = Comic {
                    cover: page.to_path_buf(),
                    name: dir.path().to_string_lossy().to_string(),
                    path: dir.path().to_path_buf(),
                };
                comics.push(comic);
            }
        }
    }

    comics.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());

    Ok(comics)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let opts = Opts::parse();

    let static_route = warp::path("static").and(warp::fs::dir(opts.data_dir));

    let router = static_route;

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
        assert_eq!(2, comics.len());

        let comic = comics.get(0).unwrap();
        assert_eq!(
            "./data/comic01/001.png",
            comic.cover.to_string_lossy().to_string()
        );

        let comic = comics.get(1).unwrap();
        assert_eq!(
            "./data/comic02/002.png",
            comic.cover.to_string_lossy().to_string()
        );
    }
}
