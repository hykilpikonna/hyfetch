use crate::neofetch_util::get_distro_name;
use crate::types::Backend;
use crate::utils::get_cache_path;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};

const FONT_LOGOS: &str = r#"{"Alma":"","Alpine":"","AOSC OS":"","Apple":"","Archcraft":"","ArchLabs":"","Arch":"","Arco":"","Arduino":"","Artix":"","Awesome WM":"","Big":"","bspwm":"","Budgie":"","CentOS":"","Cinnamon":"","Codeberg":"","CoreOS":"","Crystal":"","Debian":"","Deepin":"","Devuan":"","Docker":"","dwm":"","elementary OS":"","Endeavour OS":"","Enlightenment":"","F-droid":"","Fedora":"","Fedora (inverse)":"","Ferris":"","Flathub":"","Fluxbox":"","Forgejo":"","FOSDEM":"","FreeBSD":"","FreeCAD":"","freedesktop.org":"","Garuda":"","Gentoo":"","GIMP":"","Gitea":"","GNOME":"","GNU Guix":"","GTK":"","Hyperbola -libre":"","Hyprland":"","i3":"","illumos":"","Inkscape":"","JWM":"","Kali":"","KDE":"","KDE Neon":"","KDE Plasma":"","Kdenlive":"","KiCad":"","Krita":"","Kubuntu":"","Kubuntu (inverse)":"","Mint":"","Mint (inverse)":"","Loc-OS":"","LXDE":"","LXLE":"","LXQt":"","Mageia":"","Mandriva":"","Manjaro":"","MATE":"","mpv":"","MX":"","Neovim":"","NixOS":"","Octoprint":"","OpenBSD":"","OpenSCAD":"","OpenSUSE":"","OSH":"","OSHWA":"","OSI":"","Parabola -libre":"","Parrot OS":"","Pop!_OS":"","PostmarketOS":"","Prusa Slicer":"","Puppy":"","Qt":"","Qtile":"","QubesOS":"","Raspberry pi":"","Red Hat":"","RepRap":"","RISC-V":"","Rocky":"","Sabayon":"","Slackware":"","Slackware (inverse)":"","Snappy":"","Solus":"","Sway":"","Tails":"","Thunderbird":"","Tor Browser":"","Trisquel":"","Tux":"","Ubuntu":"","Ubuntu (inverse)":"","Vanilla OS":"","Void":"","VS Codium":"","Wayland":"","Wikimedia":"","Xero":"","XFCE":"","Xmonad":"","Xorg":"","Zorin OS":"","Windows":""}"#;

pub fn get_font_logo(backend: Backend) -> Result<String> {
    // Check if the cache file exists and return its contents if it does
    let cache_path = get_cache_path().context("Failed to get cache path")?.join("font_logo");
    if cache_path.exists() {
        let mut cached_logo = String::new();
        File::open(cache_path).context("Failed to open cache file")?
            .read_to_string(&mut cached_logo).context("Failed to read from cache file")?;
        return Ok(cached_logo);
    }

    // Deserialize the JSON into a HashMap
    let font_logos: HashMap<String, String> = serde_json::from_str::<HashMap<String, String>>(FONT_LOGOS)
        .context("Failed to deserialize font logos JSON file")?
        .into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();

    // Get the distro name
    let distro = get_distro_name(backend).context("Failed to get distro name")?.to_lowercase();

    // Find the most likely matching distro from font_logos
    let matched_distro = font_logos.keys().find(|&k| distro.contains(k))
        .or_else(|| font_logos.keys().find(|k| k.contains(&distro)))
        .or_else(|| font_logos.keys().find(|k| k.split_whitespace().any(|part| distro.contains(part))))
        .ok_or_else(|| anyhow::anyhow!("No font logo found for distro: {distro}. The supported logos are in https://github.com/Lukas-W/font-logos"))?;

    let logo = font_logos.get(matched_distro).unwrap();

    // Create parent directories for the cache if they don't exist
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).context("Failed to create cache directory")?;
    }

    // Write the logo to the cache file
    let mut cache_file = File::create(cache_path).context("Failed to create cache file")?;
    cache_file.write_all(logo.as_bytes()).context("Failed to write logo to cache file")?;

    Ok(logo.clone())
}
