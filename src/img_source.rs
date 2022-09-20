use std::{fs, path::PathBuf};

use calloop::{
    channel::{self, channel},
    LoopHandle,
};
use cosmic_bg_config::CosmicBgImgSource;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::CosmicBg;

pub fn img_source(
    bg_sources: Vec<CosmicBgImgSource>,
    handle: LoopHandle<CosmicBg>,
) -> anyhow::Result<()> {
    // Channel<(CosmicBgImgSource, Vec<PathBuf>)>
    let sources: Vec<PathBuf> = bg_sources
        .iter()
        .cloned()
        .filter_map(|source| source.try_into().ok())
        .collect();

    if sources.is_empty() {
        anyhow::bail!("Nothing to watch");
    }

    for (cosmic_source, path_source) in bg_sources.iter().zip(sources) {
        let (notify_tx, notify_rx) = channel();
        let cosmic_source_clone = cosmic_source.clone();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                if let Ok(e) = res {
                    let _ = notify_tx.send((cosmic_source_clone.clone(), e));
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => anyhow::bail!("Failed to create the watcher"),
        };

        if let Ok(m) = fs::metadata(&path_source) {
            if m.is_dir() {
                let _ = watcher.watch(&path_source, RecursiveMode::Recursive);
            } else if m.is_file() {
                let _ = watcher.watch(&path_source, RecursiveMode::NonRecursive);
            }
        }

        let _ = handle
            .insert_source(notify_rx, |e, _, state| {
                match e {
                    // TODO Rename handling?
                    channel::Event::Msg((source, event)) => match event.kind {
                        notify::EventKind::Create(_) => {
                            for w in state.wallpapers.iter_mut().filter(|w| w.source == source) {
                                for p in &event.paths {
                                    if !w.image_queue.contains(p) {
                                        w.image_queue.push_front(p.into());
                                    }
                                }
                                w.image_queue.retain(|p| !event.paths.contains(p));
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            for w in state.wallpapers.iter_mut().filter(|w| w.source == source) {
                                w.image_queue.retain(|p| !event.paths.contains(p));
                            }
                        }
                        _ => {}
                    },
                    channel::Event::Closed => todo!(),
                }
            })
            .map(|_| {})
            .map_err(|err| anyhow::anyhow!("{}", err));
    }
    Ok(())
}