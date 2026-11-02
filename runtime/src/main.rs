mod audio_output;
mod soundmanager;
mod soundguestprobe;
use std::{env, fs, process};
mod alerts;
mod appleevents;
mod callbacks;
mod colorprobe;
mod colors;
mod randomprobe;
mod getkeysprobe;
mod oseventprobe;
mod mmumodeprobe;
mod hgetvinfoprobe;
mod penpatprobe;
mod indresourceprobe;
mod newdialogprobe;
mod modal;
mod modalprobe;
mod lineprobe;
mod truncation;
mod truncationprobe;
mod dialogtextprobe;
mod boxprobe;
mod roundrect;
mod roundprobe;
mod mouseprobe;
mod flushfileprobe;
mod stilldownprobe;
mod countresourceprobe;
mod legacycolorprobe;
mod controls;
mod cursors;
mod pictures;
mod pictprobe;
mod cursorprobe;
mod damageprobe;
mod deviceprobe;
mod driverprobe;
mod dialogs;
mod environment;
mod events;
mod files;
mod fonts;
#[cfg(feature = "gui")]
mod frontend;
mod icons;
mod inverseprobe;
mod handcloneprobe;
mod mac_roman;
mod machine;
mod managers;
mod menus;
mod originprobe;
mod pixmapprobe;
mod offdeviceprobe;
mod processes;
mod quickdraw;
mod regions;
mod resfiles;
mod resource;
mod scrap;
mod saneprobe;
mod scripts;
mod segments;
mod sound;
mod standardfile;
mod stringutils;
mod styledtext;
mod textedit;
mod timemanager;
mod timing;
mod vbl;
mod vblprobe;
mod vendor;
mod windows;
fn main() {
    if env::args().nth(1).as_deref() == Some("--audio-vector") {
        if let Err(error) = audio_output::vector(&env::args().collect::<Vec<_>>()) {
            eprintln!("audio vector: {error}");
            process::exit(1);
        }
        return;
    }
    if env::args().nth(1).as_deref() == Some("--catalog") {
        let result = (|| -> Result<serde_json::Value, String> {
            let path = env::args().nth(2).ok_or("missing resource fork path")?;
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let resources = resource::parse(&bytes)?;
            let mut counts = std::collections::BTreeMap::<String, usize>::new();
            let mut sizes = std::collections::BTreeMap::<String, usize>::new();
            let mut entries = Vec::new();
            for r in resources {
                let kind = String::from_utf8_lossy(&r.kind).into_owned();
                *counts.entry(kind.clone()).or_default() += 1;
                *sizes.entry(kind.clone()).or_default() += r.data.len();
                let name = r.name.map(|v| {
                    v.iter()
                        .map(|b| mac_roman::MAC_ROMAN[*b as usize])
                        .collect::<String>()
                });
                entries.push(serde_json::json!({"kind":kind,"id":r.id,"bytes":r.data.len(),"attributes":r.attributes,"name":name,"prefix_hex":r.data.iter().take(16).map(|b|format!("{b:02x}")).collect::<String>()}));
            }
            Ok(
                serde_json::json!({"input":path,"fork_bytes":bytes.len(),"counts":counts,"resource_bytes":sizes,"entries":entries,"scope":"Resource metadata and raw prefixes only; no executed-code/trap coverage claim"}),
            )
        })();
        match result {
            Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if env::args().nth(1).as_deref() == Some("--probe") {
        let result = match env::args().nth(2).as_deref() {
            Some("sound-guest-v1") => soundguestprobe::run(),
            Some("sane-cos-runtime-raw-v1") => saneprobe::cos_probe(),
            Some("sane-tan-runtime-raw-v1") => saneprobe::tan_probe(),
            Some("sane-narrow-runtime-raw-v1") => saneprobe::narrow_probe(),
            Some("sane-round-runtime-raw-v1") => saneprobe::round_probe(),
            Some("sane-compare-runtime-raw-v1") => saneprobe::comparison_formats_probe(),
            Some("sane-mixedadd-runtime-raw-v1") => saneprobe::mixed_add_probe(),
            Some("sane-mixed-runtime-raw-v1") => saneprobe::mixed_probe(),
            Some("sane-basicx-runtime-raw-v1") => saneprobe::basic_probe(),
            Some("sane-cmp-runtime-raw-v1") => saneprobe::comparison_probe(),
            Some("sane-env-runtime-raw-v1") => saneprobe::environment_probe(),
            Some("sane-class-runtime-raw-v1") => saneprobe::classification_probe(),
            Some("sane-extended-runtime-raw-v1") => saneprobe::extended_probe(),
            Some("sane-sine-runtime-raw-v1") => saneprobe::sine_probe(),
            Some("sane-runtime-raw-v1") => saneprobe::probe(),
            Some("sane-mul-runtime-raw-v1") => saneprobe::multiplication_probe(),
            Some("sane-div-runtime-raw-v1") => saneprobe::division_probe(),
            Some("sane-integer-runtime-raw-v1") => saneprobe::integer_probe(),
            Some("sane-widen-runtime-raw-v1") => saneprobe::widening_probe(),
            Some("sane-single-widen-runtime-raw-v1") => saneprobe::single_widening_probe(),
            Some("sane-long-widen-runtime-raw-v1") => saneprobe::long_widening_probe(),
            Some("sane-small-integer-power-runtime-raw-v1") => saneprobe::small_integer_power_probe(),
            Some("sane-convert-runtime-raw-v1") => saneprobe::conversion_probe(),
            Some("hgetvinfo-runtime-raw-v1") => hgetvinfoprobe::probe(),
            Some("pen-pattern-runtime-raw-v1") => penpatprobe::probe(),
            Some("ind-resource-runtime-raw-v1") => indresourceprobe::probe(),
            Some("new-dialog-details-runtime-raw-v1") => newdialogprobe::probe(),
            Some("modal-dialog-runtime-raw-v1") => modalprobe::probe(),
            Some("qd-line-runtime-raw-v1") => lineprobe::probe(),
            Some("text-truncation-runtime-raw-v1") => truncationprobe::probe(),
            Some("dialog-item-text-runtime-raw-v1") => dialogtextprobe::probe(),
            Some("text-box-runtime-raw-v3") => boxprobe::probe(),
            Some("round-rect-runtime-raw-v1") => roundprobe::probe(),
            Some("mouse-coupling-runtime-raw-v1") => mouseprobe::probe(),
            Some("mmu-mode-runtime-raw-v1") => mmumodeprobe::probe(),
            Some("os-events-runtime-raw-v2") => oseventprobe::probe(),
            Some("get-keys-runtime-raw-v1") => getkeysprobe::probe(),
            Some("qd-random-runtime-raw-v1") => randomprobe::probe(),
            Some("flush-file-runtime-raw-v1") => flushfileprobe::probe(),
            Some("still-down-runtime-raw-v1") => stilldownprobe::probe(),
            Some("count-resources-runtime-raw-v1") => countresourceprobe::probe(),
            Some("legacy-color-runtime-raw-v1") => legacycolorprobe::probe(false),
            Some("color-read-runtime-raw-v1") => legacycolorprobe::probe(true),
            Some("inverse-runtime-raw-v1") => inverseprobe::probe(),
            Some("inverse-four-runtime-raw-v1") => inverseprobe::probe_four(),
            Some("hand-clone-v1") => handcloneprobe::probe(),
            Some("off-device-runtime-raw-v1") => offdeviceprobe::probe(),
            Some("max-device-v1") => deviceprobe::probe(),
            Some("origin-v1") => originprobe::probe(),
            Some("vbl-runtime-raw-v1") => vblprobe::probe(),
            Some("window-background-v1") => machine::window_background_probe(),
            Some("window-colors-v1") => machine::window_colors_probe(),
            Some("move-high-raw-v4") => machine::move_high_probe(),
            Some("indexed-color-v1") => colorprobe::probe(),
            Some("time-manager-raw-v2") => machine::time_manager_probe(),
            Some("alerts-v1") => machine::alerts_probe(),
            Some("sound-no-output-v1") => sound::no_output_probe(),
            Some("owned-68k-v1") => machine::owned68k_probe(false),
            Some("owned-68k-execution-v1") => machine::owned68k_probe(true),
            Some("addressing-v1") => environment::addressing_probe(),
            Some("sound-version-abi-v1") => environment::sound_version_probe(),
            Some("catalog-semantics-v1") => machine::cataloginfo_probe(),
            Some("fcb-semantics-v1") => machine::fcbinfo_probe(),
            Some("open-df-runtime-raw-v2") => machine::opendf_probe(),
            Some("path-files-pb-semantics-v1") => machine::pathfiles_probe(),
            Some("control-thumb-edges-semantics-v1") => machine::controlthumbedges_probe(),
            Some("control-thumb-semantics-v1") => machine::controlthumb_probe(),
            Some("control-action-v1") => machine::controlaction_probe(),
            Some("te-double-v1") => machine::tedouble_probe(),
            Some("control-hit-v1") => machine::controlhit_probe(),
            Some("window-tracking-v1") => machine::windowtracking_probe(),
            Some("window-geometry-v1") => machine::windowgeometry_probe(),
            Some("style-use-v1") => machine::styleuse_probe(),
            Some("handle-state-v1") => machine::handlestate_probe(),
            Some("doc-lifecycle-v1") => machine::doclifecycle_probe(),
            Some("style-scrap-v1") => machine::stylescrap_probe(),
            Some("res-io-reload-v1") => machine::resio_reload_probe(),
            Some("get-color-table-raw-v1") => machine::getctable_probe(),
            Some("app-resource-release-v1") => machine::apprelease_probe(),
            Some("res-io-v1") => machine::resio_probe(),
            Some("res-create-v1") => machine::rescreate_probe(),
            Some("icon-plot-v1") => machine::iconplot_probe(),
            Some("user-items-v1") => machine::useritems_probe(),
            Some("delay-v1") => machine::delay_probe(),
            Some("script-context-v1") => machine::scriptcontext_probe(),
            Some("standard-filter-v1") => machine::standardfilter_probe(),
            Some("modal-filter-v1") => machine::modalfilter_probe(),
            Some("font-globals-v1") => machine::fontglobals_probe(),
            Some("font-availability-v1") => machine::fontavailability_probe(),
            Some("string-compare-v1") => machine::stringcompare_probe(),
            Some("resource-enum-v1") => machine::resourceenum_probe(),
            Some("te-auto-view-v1") => machine::teautoview_probe(),
            Some("te-outline-refresh-v1") => machine::teoutline_probe(),
            Some("te-continuous-control-v1") => machine::tecontinuous_probe(true),
            Some("te-continuous-v1") => machine::tecontinuous_probe(false),
            Some("te-arrows-v1") => machine::tearrows_probe(),
            Some("style-queries-v1") => machine::stylequeries_probe(),
            Some("styled-text-v1") => machine::styledtext_probe(false),
            Some("styled-text-variant-v1") => machine::styledtext_probe(true),
            Some("window-resources-v1") => machine::windowresources_probe(false),
            Some("window-resource-flow-v1") => machine::windowresources_probe(true),
            Some("apple-event-handlers-v1") => machine::aehandlers_probe(),
            Some("menu-items-v1") => machine::menuitems_probe(),
            Some("menu-semantics-v1") => machine::menusemantics_probe(),
            Some("pict-bitmap-runtime-raw-v1") => pictprobe::probe(),
            Some("cursor-state-runtime-raw-v1") => cursorprobe::state_probe(),
            Some("cursor-motion-runtime-raw-v1") => cursorprobe::motion_probe(),
            Some("ctab-changed-raw-v1") => machine::ctabchanged_probe(),
            Some("color-seed-raw-v1") => machine::ctseed_probe(),
            Some("window-damage-raw-v2") => damageprobe::probe(),
            Some("empty-driver-raw-v1") => driverprobe::probe(),
            Some("pixmap-raw-v1") => pixmapprobe::probe(),
            Some("courier-font-raw-v1") => machine::courier_probe(),
            Some("font-catalog-v1") => machine::fontcatalog_probe(),
            Some("font-profile-v1") => machine::fontprofile_probe(),
            Some("system-styles-v1") => machine::systemstyle_probe(),
            Some("menu-bars-v1") => machine::menubar_probe(),
            Some("gestalt-v1") => machine::gestalt_probe(),
            Some("environs-v1") => machine::environs_probe(),
            Some("segments-v1") => machine::segment_probe(),
            Some("app-source-v1") => machine::appsource_probe(),
            Some("dialog-params-v1") => machine::dialogparams_probe(),
            Some("file-delete-v1") => machine::filedelete_probe(),
            Some("save-confirm-v1") => machine::saveconfirm_probe(),
            Some("processes-v1") => machine::process_probe(),
            Some("clipboard-v1") => machine::clipboard_probe(),
            Some("core-v1") => machine::core_probe(),
            Some("quickdraw-v1") => machine::quickdraw_probe(),
            Some("windows-v1") => machine::window_probe(),
            Some("memory-flags-v1") => machine::memory_flags_probe(),
            Some("regions-v1") => machine::region_probe(),
            Some("color-ports-v1") => machine::color_port_probe(),
            Some("textedit-v1") => machine::textedit_probe(),
            Some("text-layout-v1") => machine::textlayout_probe(),
            Some("controls-v1") => machine::control_probe(),
            Some("window-flow-v1") => machine::windowflow_probe(),
            Some("files-v1") => machine::file_probe(),
            Some("res-files-v1") => machine::resfile_probe(),
            Some("text-direct-v1") => machine::textdirect_probe(),
            Some("hit-test-v1") => machine::hittest_probe(),
            Some("title-metrics-v1") => machine::titlemetrics_probe(),
            Some("title-bounds-v1") => machine::titlebounds_probe(),
            Some("title-pairs-v1") => machine::titlepairs_probe(),
            _ => Err("unknown probe".into()),
        };
        match result {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        }
        return;
    }
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: classic-runtime RESOURCE_FORK");
        process::exit(2)
    });
    let result = fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| {
        let resources = resource::parse(&bytes)?;
        for r in &resources {
            println!(
                "{} id={} bytes={} compressed={}",
                String::from_utf8_lossy(&r.kind),
                r.id,
                r.data.len(),
                r.attributes & 1 != 0
            );
        }
        if !resources.iter().any(|r| r.kind == *b"CODE" && r.id == 0) {
            return Err("No 68K CODE 0 jump table; this is not a supported executable yet".into());
        }
        let args: Vec<String> = env::args().collect();
        if let Some(index) = args.iter().position(|a| a == "--dump-code") {
            let dir =
                std::path::Path::new(args.get(index + 1).ok_or("--dump-code needs a directory")?);
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            for r in resources.iter().filter(|r| r.kind == *b"CODE") {
                let decoded = vendor::compressed::decompress_if_needed(r.attributes, r.data)
                    .map_err(|e| format!("{e:?}"))?;
                fs::write(
                    dir.join(format!("code{}.bin", r.id)),
                    decoded.as_deref().unwrap_or(r.data),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if env::args().any(|a| a == "--trace" || a == "--gui") {
            machine::trace(&resources, &bytes)?;
        }
        Ok(())
    });
    if let Err(e) = result {
        eprintln!("{e}");
        process::exit(1);
    }
}
