//! Job Object Windows « kill on close » : les processus de fond meurent avec l'application,
//! même en cas de plantage ou de fermeture brutale.
use std::process::Child;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "procs_unix.rs"]
mod unix;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use unix::{acquire_single_instance, local_hhmm, local_stamp, terminate_now, wait_for_exit};

#[cfg(target_os = "macos")]
#[path = "procs_mac.rs"]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::{
    message_box, notify, open_folder, pick_folder, set_startup, startup_enabled, user_locale,
};

#[cfg(target_os = "linux")]
#[path = "procs_linux.rs"]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    autostart_file, ensure_start_menu_shortcut, menu_files, message_box, notify, open_folder,
    pick_folder, set_startup, startup_enabled, user_locale,
};

/// Programme à lancer pour relancer l'application (redémarrage, repli X11). Linux : le fichier AppImage s'il y en a
/// un, car l'exécutable en cours est dans l'image montée, qui disparaît quand ce processus se ferme.
pub fn relaunch_exe() -> std::io::Result<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(p) = linux::launcher_exe() {
        return Ok(p);
    }
    std::env::current_exe()
}

#[cfg(windows)]
mod imp {
    use super::Child;
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    struct Job(HANDLE);
    // SAFETY : un handle de Job Object peut être utilisé depuis n'importe quel thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<&'static Job> {
        JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(h))
        })
        .as_ref()
    }

    pub fn attach(child: &Child) {
        if let Some(j) = job() {
            // SAFETY : handle de processus valide tant que `child` existe.
            let r = unsafe { AssignProcessToJobObject(j.0, HANDLE(child.as_raw_handle())) };
            if r.is_err() {
                crate::logging::log("Job Object : affectation du processus impossible");
            }
        }
    }
}

#[cfg(windows)]
pub fn attach(child: &Child) {
    imp::attach(child);
}

#[cfg(not(windows))]
pub fn attach(_child: &Child) {}

/// Vrai si Windows est en mode « contraste élevé » (thème de contraste activé dans Paramètres > Accessibilité).
#[cfg(windows)]
pub fn system_high_contrast() -> bool {
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut hc = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // SAFETY : `hc` est une structure valide, de la taille annoncée dans `cbSize`, pour toute la durée de l'appel.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some(&mut hc as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && (hc.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
}

#[cfg(not(windows))]
pub fn system_high_contrast() -> bool {
    false
}

/// Rectangle du bureau virtuel (tous les écrans) en pixels : `(gauche, haut, droite, bas)`.
#[cfg(windows)]
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    // SAFETY : simple lecture de métriques système.
    unsafe {
        let (x, y) = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        (
            x,
            y,
            x + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            y + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

#[cfg(not(windows))]
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    (0, 0, 100_000, 100_000)
}

/// Vrai si la barre de titre d'une fenêtre placée en `pos` (points) reste atteignable à l'écran.
pub fn position_is_visible(pos: [f32; 2], ppp: f32) -> bool {
    let (l, t, r, b) = virtual_screen();
    let (x, y) = ((pos[0] * ppp) as i32, (pos[1] * ppp) as i32);
    x > l - 200 && x < r - 120 && y >= t - 10 && y < b - 80
}

// ---------- Identité, instance unique, écrans ----------

/// Identifiant d'application Windows : regroupe la fenêtre sous une seule icône dans la barre des tâches.
#[cfg(windows)]
pub fn set_app_id() {
    use windows::core::w;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    // SAFETY : chaîne statique valide.
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(w!("Velka.App")) };
}

#[cfg(not(windows))]
pub fn set_app_id() {}

/// Identifiant d'application (voir `set_app_id`).
#[cfg(windows)]
const APP_ID: &str = "Velka.App";

/// Icône de l'application (nouveau logo), la même que celle de l'exécutable.
#[cfg(windows)]
const APP_ICO: &[u8] = include_bytes!("../assets/velka.ico");

/// Raccourci « Velka » du menu Démarrer portant l'identifiant d'application : sans lui, Windows affiche
/// « Velka.App » sans logo en tête des notifications. Un raccourci existant (installateur) garde sa cible
/// et reçoit seulement l'identifiant ; sinon la version portable en crée un vers l'exécutable en cours.
/// Les versions de développement ne créent rien.
#[cfg(windows)]
pub fn ensure_start_menu_shortcut() {
    // Instance isolée (essais) : le raccourci de l'installation habituelle n'est jamais touché.
    if crate::logging::isolated_dir().is_some() {
        return;
    }
    let Some(dir) = directories::BaseDirs::new().map(|d| {
        d.config_dir()
            .join(r"Microsoft\Windows\Start Menu\Programs")
    }) else {
        return;
    };
    let lnk = dir.join("Velka.lnk");
    let exists = lnk.exists();
    if !exists && cfg!(debug_assertions) {
        return;
    }
    // Logo actuel à côté des données : le raccourci l'affiche (menu Démarrer, notifications) même s'il
    // vise un exécutable plus ancien.
    let icon = crate::logging::data_dir().join("velka.ico");
    if std::fs::read(&icon).ok().as_deref() != Some(APP_ICO)
        && std::fs::write(&icon, APP_ICO).is_err()
    {
        return;
    }
    match write_shortcut(&lnk, exists, &icon) {
        Ok(true) => crate::logging::log("Raccourci du menu Démarrer mis à jour (notifications)"),
        Ok(false) => {}
        Err(e) => crate::logging::log(&format!("Raccourci du menu Démarrer : {e}")),
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn ensure_start_menu_shortcut() {}

/// Écrit l'identifiant et l'icône dans le raccourci (et la cible s'il est nouveau). `Ok(false)` si rien n'a changé.
#[cfg(windows)]
fn write_shortcut(
    lnk: &std::path::Path,
    exists: bool,
    icon: &std::path::Path,
) -> windows::core::Result<bool> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READWRITE,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let path = HSTRING::from(lnk.as_os_str());
    // SAFETY : COM initialisé pour ce fil (fil dédié) ; toutes les interfaces viennent de `CoCreateInstance`
    // et les chaînes passées restent valides pendant les appels.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let file: IPersistFile = link.cast()?;
        if exists {
            file.Load(&path, STGM_READWRITE)?;
        } else {
            let exe = std::env::current_exe().map_err(|_| windows::core::Error::empty())?;
            link.SetPath(&HSTRING::from(exe.as_os_str()))?;
            if let Some(d) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(d.as_os_str()))?;
            }
            link.SetDescription(&HSTRING::from("Velka"))?;
        }
        let icon_path = icon.to_string_lossy().to_string();
        let mut buf = [0u16; 520];
        let mut idx = 0i32;
        let icon_ok = link.GetIconLocation(&mut buf, &mut idx).is_ok() && {
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..n]).eq_ignore_ascii_case(&icon_path) && idx == 0
        };
        let store: IPropertyStore = link.cast()?;
        if exists
            && icon_ok
            && store
                .GetValue(&PKEY_AppUserModel_ID)
                .is_ok_and(|v| v.to_string() == APP_ID)
        {
            return Ok(false);
        }
        link.SetIconLocation(&HSTRING::from(icon_path.as_str()), 0)?;
        store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(APP_ID))?;
        store.Commit()?;
        file.Save(&path, true)?;
    }
    Ok(true)
}

/// Désinstallation depuis l'application : le logo du dossier de données va disparaître. Le raccourci de la version
/// portable est supprimé ; un autre (installateur) reprend l'icône de son exécutable.
#[cfg(windows)]
pub fn forget_start_menu_shortcut() {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READWRITE,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let Some(lnk) = directories::BaseDirs::new().map(|d| {
        d.config_dir()
            .join(r"Microsoft\Windows\Start Menu\Programs\Velka.lnk")
    }) else {
        return;
    };
    if !lnk.exists() {
        return;
    }
    let exe = std::env::current_exe().ok();
    let installed = exe.as_ref().is_some_and(|e| {
        e.to_string_lossy()
            .to_lowercase()
            .contains(r"\programs\velka\")
    });
    // SAFETY : COM initialisé pour ce fil ; interfaces issues de `CoCreateInstance`, chaînes valides pendant les appels.
    let res: windows::core::Result<()> = unsafe {
        (|| {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let file: IPersistFile = link.cast()?;
            let path = HSTRING::from(lnk.as_os_str());
            file.Load(&path, STGM_READWRITE)?;
            let mut buf = [0u16; 520];
            link.GetPath(&mut buf, std::ptr::null_mut(), 0)?;
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let target = String::from_utf16_lossy(&buf[..n]);
            let ours = exe
                .as_ref()
                .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(&target));
            if ours && !installed {
                let _ = std::fs::remove_file(&lnk);
            } else {
                link.SetIconLocation(&HSTRING::from(target.as_str()), 0)?;
                file.Save(&path, true)?;
            }
            Ok(())
        })()
    };
    if let Err(e) = res {
        crate::logging::log(&format!("Raccourci du menu Démarrer : {e}"));
    }
}

#[cfg(not(windows))]
pub fn forget_start_menu_shortcut() {}

/// Fenêtre Windows de choix d'un dossier (à appeler depuis un fil dédié : elle bloque jusqu'à la réponse).
/// `start` : dossier affiché à l'ouverture. `None` si l'utilisateur annule ou si la fenêtre n'a pas pu s'ouvrir.
#[cfg(windows)]
pub fn pick_folder(start: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };
    // SAFETY : COM initialisé pour ce fil (fil dédié) ; interfaces issues de `CoCreateInstance`, chaînes valides pendant
    // les appels, chaîne renvoyée par Windows libérée avec `CoTaskMemFree` après copie.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
            .ok()?;
        if let Some(dir) = start {
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                &HSTRING::from(dir.as_os_str()),
                None,
            ) {
                let _ = dialog.SetFolder(&item);
            }
        }
        dialog.Show(None).ok()?;
        let item = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        path.map(std::path::PathBuf::from)
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn pick_folder(_start: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    None
}

/// Vrai pour la première instance ; faux si Velka tourne déjà.
#[cfg(windows)]
pub fn acquire_single_instance() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    // SAFETY : le handle est volontairement conservé jusqu'à la fin du processus.
    unsafe {
        match CreateMutexW(None, true, w!("Local\\Velka.SingleInstance")) {
            Ok(handle) => {
                let first = GetLastError() != ERROR_ALREADY_EXISTS;
                if !first {
                    // Poignée rendue : sinon ce processus garderait le verrou en vie et une nouvelle tentative (relance
                    // après mise à jour) échouerait même une fois l'autre instance fermée.
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                }
                first
            }
            Err(_) => true, // en cas de doute, on laisse démarrer
        }
    }
}

/// Attend la fin du processus `pid`, au plus `max`. Rend la main tout de suite s'il n'existe plus ou s'il est inaccessible.
#[cfg(windows)]
pub fn wait_for_exit(pid: u32, max: std::time::Duration) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    // SAFETY : la poignée est obtenue puis fermée dans cette fonction ; `WaitForSingleObject` ne fait que l'attendre.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return;
        };
        let _ = WaitForSingleObject(h, u32::try_from(max.as_millis()).unwrap_or(u32::MAX));
        let _ = CloseHandle(h);
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn wait_for_exit(_pid: u32, _max: std::time::Duration) {}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn acquire_single_instance() -> bool {
    true
}

/// Titre de la fenêtre principale.
#[cfg(windows)]
const WINDOW_TITLE: &str = "Velka";

/// Deux chemins désignent-ils un programme de même nom (sans tenir compte de la casse) ?
#[cfg(windows)]
fn same_exe_name(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.file_name(), b.file_name()) {
        (Some(x), Some(y)) => x
            .to_string_lossy()
            .eq_ignore_ascii_case(&y.to_string_lossy()),
        _ => false,
    }
}

#[cfg(windows)]
struct WindowSearch {
    /// `true` : la fenêtre de ce processus ; `false` : celle d'un autre processus `velka.exe` (instance déjà ouverte).
    own: bool,
    found: Option<windows::Win32::Foundation::HWND>,
}

/// Le processus `pid` exécute-t-il un programme de même nom que le nôtre ?
#[cfg(windows)]
fn is_same_program(pid: u32) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let Ok(me) = std::env::current_exe() else {
        return false;
    };
    // SAFETY : la poignée obtenue est fermée avant de rendre la main ; le tampon et `len` sont valides pendant l'appel.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(h);
        ok && same_exe_name(
            &std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])),
            &me,
        )
    }
}

#[cfg(windows)]
unsafe extern "system" fn window_search_cb(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId};
    // SAFETY : `lparam` est le pointeur vers le `WindowSearch` de `find_window`, vivant pendant toute l'énumération.
    let st = unsafe { &mut *(lparam.0 as *mut WindowSearch) };
    let mut title = [0u16; 32];
    // SAFETY : tampon valide pour la durée de l'appel.
    let n = unsafe { GetWindowTextW(hwnd, &mut title) };
    if n <= 0 || String::from_utf16_lossy(&title[..n as usize]) != WINDOW_TITLE {
        return true.into();
    }
    let mut pid = 0u32;
    // SAFETY : `pid` est valide pour la durée de l'appel.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let mine = pid == unsafe { GetCurrentProcessId() };
    if (st.own && mine) || (!st.own && !mine && is_same_program(pid)) {
        st.found = Some(hwnd);
        return false.into(); // trouvée : on arrête l'énumération
    }
    true.into()
}

/// Fenêtre principale titrée « Velka » qui appartient bien à Velka : la nôtre (`own`) ou celle d'une instance
/// déjà ouverte. Une autre fenêtre de même titre (navigateur, document…) n'est jamais prise.
#[cfg(windows)]
fn find_window(own: bool) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
    let mut st = WindowSearch { own, found: None };
    // SAFETY : `st` reste valide pendant toute l'énumération (appel synchrone).
    unsafe {
        let _ = EnumWindows(Some(window_search_cb), LPARAM((&raw mut st) as isize));
    }
    st.found
}

/// Notre fenêtre, retrouvée une seule fois puis gardée : `find_window` parcourt toutes les fenêtres du système
/// (et lit leur titre), ce qui ralentissait chaque bascule de plein écran.
#[cfg(windows)]
fn own_window() -> Option<windows::Win32::Foundation::HWND> {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
    static CACHE: AtomicIsize = AtomicIsize::new(0);
    let cached = CACHE.load(Ordering::Relaxed);
    if cached != 0 {
        let hwnd = HWND(cached as *mut std::ffi::c_void);
        // SAFETY : simple vérification qu'une poignée existe encore.
        if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Some(hwnd);
        }
    }
    let hwnd = find_window(true)?;
    CACHE.store(hwnd.0 as isize, Ordering::Relaxed);
    Some(hwnd)
}

/// Ramène la fenêtre de l'instance déjà ouverte au premier plan.
#[cfg(windows)]
pub fn focus_existing_window() {
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
    // SAFETY : appels Win32 simples sur une fenêtre de l'instance déjà ouverte.
    unsafe {
        if let Some(hwnd) = find_window(false) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
pub fn focus_existing_window() {}

/// Échelle d'affichage du système (1.0 = 100 %).
#[cfg(windows)]
pub fn system_scale() -> f32 {
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    // SAFETY : simple lecture.
    (unsafe { GetDpiForSystem() } as f32 / 96.0).max(1.0)
}

#[cfg(not(windows))]
pub fn system_scale() -> f32 {
    1.0
}

/// Rectangles des écrans en pixels `(gauche, haut, droite, bas)`, écran principal en premier.
#[cfg(windows)]
pub fn monitor_rects() -> Vec<(i32, i32, i32, i32)> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };

    unsafe extern "system" fn cb(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY : `data` pointe vers le Vec créé ci-dessous, vivant pendant l'énumération.
        let list = unsafe { &mut *(data.0 as *mut Vec<(bool, RECT)>) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(m, &mut info) }.as_bool() {
            list.push((info.dwFlags & 1 != 0, info.rcMonitor));
        }
        BOOL(1)
    }

    let mut list: Vec<(bool, RECT)> = Vec::new();
    // SAFETY : voir `cb`.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut list as *mut _ as isize));
    }
    list.sort_by_key(|(primary, r)| (!*primary, r.left, r.top));
    list.into_iter()
        .map(|(_, r)| (r.left, r.top, r.right, r.bottom))
        .collect()
}

#[cfg(not(windows))]
pub fn monitor_rects() -> Vec<(i32, i32, i32, i32)> {
    Vec::new()
}

/// Heure locale « HH:MM » (horodatage du chat).
#[cfg(windows)]
pub fn local_hhmm() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY : simple lecture de l'heure système.
    let t = unsafe { GetLocalTime() };
    format!("{:02}:{:02}", t.wHour, t.wMinute)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn local_hhmm() -> String {
    String::new()
}

/// Date et heure locales pour un nom de fichier : `2026-11-20_14-03-22`.
#[cfg(windows)]
pub fn local_stamp() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY : simple lecture de l'heure système.
    let t = unsafe { GetLocalTime() };
    format!(
        "{}-{:02}-{:02}_{:02}-{:02}-{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn local_stamp() -> String {
    commun::now_secs().to_string()
}

/// Clé « Run » de l'utilisateur (démarrage avec Windows) et nom de notre valeur.
#[cfg(windows)]
const RUN_SUBKEY: windows::core::PCWSTR =
    windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
#[cfg(windows)]
const RUN_VALUE: windows::core::PCWSTR = windows::core::w!("Velka");

/// Chemin complet d'un programme du dossier système de Windows (`System32`). Jamais un nom nu : Windows chercherait
/// d'abord dans le dossier de l'application, qui est inscriptible par l'utilisateur.
#[cfg(windows)]
pub fn system_exe(name: &str) -> std::path::PathBuf {
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 260];
    // SAFETY : tampon valide ; la fonction renvoie le nombre de caractères écrits (0 en cas d'échec).
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    let dir = if n > 0 && n < buf.len() {
        std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]))
    } else {
        std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()),
        )
        .join("System32")
    };
    dir.join(name)
}

/// Chemin complet d'un programme du dossier de Windows (par exemple `explorer.exe`).
#[cfg(windows)]
pub fn windows_exe(name: &str) -> std::path::PathBuf {
    use windows::Win32::System::SystemInformation::GetWindowsDirectoryW;
    let mut buf = [0u16; 260];
    // SAFETY : voir `system_exe`.
    let n = unsafe { GetWindowsDirectoryW(Some(&mut buf)) } as usize;
    let dir = if n > 0 && n < buf.len() {
        std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]))
    } else {
        std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()),
        )
    };
    dir.join(name)
}

/// Termine le processus tout de suite, sans fermeture normale (essai de démarrage vidéo, `smoke.rs` ; fermeture de
/// l'application après un live, `VelkaApp::fast_exit`, une fois les réglages enregistrés).
#[cfg(windows)]
pub fn terminate_now() {
    use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
    // SAFETY : arrêt immédiat de ce processus ; rien n'est utilisé après.
    let _ = unsafe { TerminateProcess(GetCurrentProcess(), 0) };
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn terminate_now() {
    std::process::exit(0);
}

/// Ouvre un dossier dans l'Explorateur.
#[cfg(windows)]
pub fn open_folder(dir: &std::path::Path) {
    let _ = std::process::Command::new(windows_exe("explorer.exe"))
        .arg(dir)
        .spawn();
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn open_folder(_dir: &std::path::Path) {}

/// L'application démarre-t-elle avec Windows ? (lecture directe du registre, sans lancer `reg.exe`)
#[cfg(windows)]
pub fn startup_enabled() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    // SAFETY : chaînes constantes terminées par un zéro ; aucune donnée n'est demandée en retour.
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    r.0 == 0
}

/// Active ou retire le démarrage avec Windows (clé « Run » de l'utilisateur, sans droits administrateur).
#[cfg(windows)]
pub fn set_startup(on: bool) -> bool {
    use windows::Win32::System::Registry::{
        RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };
    if on {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let value: Vec<u16> = format!("\"{}\"", exe.display())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY : `value` est terminée par un zéro et vivante pendant l'appel ; la taille est donnée en octets.
        let r = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_SUBKEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(value.as_ptr().cast()),
                (value.len() * 2) as u32,
            )
        };
        r.0 == 0
    } else {
        // SAFETY : chaînes constantes terminées par un zéro.
        let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_SUBKEY, RUN_VALUE) };
        // Valeur déjà absente (code 2) : c'est l'état voulu.
        r.0 == 0 || r.0 == 2
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn startup_enabled() -> bool {
    false
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn set_startup(_on: bool) -> bool {
    false
}

/// Fenêtre principale (titre « Velka »), nécessaire pour la zone de notification.
#[cfg(windows)]
fn base_icon_data() -> Option<windows::Win32::UI::Shell::NOTIFYICONDATAW> {
    use windows::core::PCWSTR;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{NIF_ICON, NIF_TIP, NOTIFYICONDATAW};
    use windows::Win32::UI::WindowsAndMessaging::LoadIconW;
    let hwnd = find_window(true)?;
    // SAFETY : appels Win32 simples ; les pointeurs passés sont valides pour la durée de l'appel.
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        nid.uFlags = NIF_ICON | NIF_TIP;
        if let Ok(h) = GetModuleHandleW(None) {
            // L'icône de l'exécutable est la ressource n° 1 (voir build.rs).
            if let Ok(icon) = LoadIconW(
                Some(h.into()),
                PCWSTR(std::ptr::without_provenance::<u16>(1)),
            ) {
                nid.hIcon = icon;
            }
        }
        copy_wide(&mut nid.szTip, "Velka");
        Some(nid)
    }
}

#[cfg(windows)]
fn copy_wide(dst: &mut [u16], s: &str) {
    let max = dst.len().saturating_sub(1);
    for (slot, c) in dst.iter_mut().zip(s.encode_utf16().take(max)) {
        *slot = c;
    }
}

/// Date du dernier message envoyé : l'icône de la zone de notification n'existe que le temps de la bulle.
#[cfg(windows)]
static TRAY_SINCE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Chaîne à ouvrir quand on clique sur la dernière notification, et clic reçu (lu par l'interface).
#[cfg(windows)]
static NOTIFY_TARGET: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
#[cfg(windows)]
static NOTIFY_CLICKED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// Réveille l'interface (nouvelle image) quand un clic de notification arrive.
#[cfg(windows)]
static NOTIFY_WAKE: std::sync::Mutex<Option<Box<dyn Fn() + Send>>> = std::sync::Mutex::new(None);

/// Message envoyé par la zone de notification à la fenêtre (WM_APP + 1).
#[cfg(windows)]
const TRAY_CALLBACK: u32 = 0x8000 + 1;

/// Fournit la fonction qui réveille l'interface (à appeler une fois au démarrage).
pub fn set_notify_wake(f: impl Fn() + Send + 'static) {
    #[cfg(windows)]
    if let Ok(mut w) = NOTIFY_WAKE.lock() {
        *w = Some(Box::new(f));
    }
    #[cfg(not(windows))]
    let _ = f;
}

/// Chaîne de la notification sur laquelle l'utilisateur vient de cliquer, s'il y en a une (lue une seule fois).
pub fn take_notification_click() -> Option<String> {
    #[cfg(windows)]
    {
        NOTIFY_CLICKED.lock().ok().and_then(|mut c| c.take())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Option « Réduire dans la zone de notification à la fermeture » : l'icône reste en place au lieu de disparaître
/// après la bulle.
#[cfg(windows)]
static PERMANENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Commande choisie dans l'icône (0 aucune, `TRAY_OPEN`, `TRAY_QUIT`), lue une fois par l'interface.
#[cfg(windows)]
static TRAY_CMD: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Libellés (traduits) du menu de l'icône : « Ouvrir Velka » et « Quitter ».
#[cfg(windows)]
static TRAY_LABELS: std::sync::Mutex<(String, String)> =
    std::sync::Mutex::new((String::new(), String::new()));

/// Commandes de l'icône de la zone de notification.
pub const TRAY_OPEN: u8 = 1;
pub const TRAY_QUIT: u8 = 2;

/// Réveille l'interface (nouvelle image) après un événement de la zone de notification.
#[cfg(windows)]
fn wake_ui() {
    if let Ok(w) = NOTIFY_WAKE.lock() {
        if let Some(f) = w.as_ref() {
            f();
        }
    }
}

/// Menu du clic droit sur l'icône : renvoie la commande choisie (0 si le menu est fermé sans choix).
#[cfg(windows)]
fn tray_menu(hwnd: windows::Win32::Foundation::HWND) -> u8 {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
        TrackPopupMenu, MF_STRING, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
    };
    let (open, quit) = TRAY_LABELS.lock().map(|l| l.clone()).unwrap_or_default();
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (open, quit) = (wide(&open), wide(&quit));
    let mut pt = POINT::default();
    // SAFETY : menu créé, rempli, affiché puis détruit ici ; les chaînes (terminées par un zéro) vivent pendant les appels.
    unsafe {
        if GetCursorPos(&mut pt).is_err() {
            return 0;
        }
        let Ok(menu) = CreatePopupMenu() else {
            return 0;
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            usize::from(TRAY_OPEN),
            PCWSTR(open.as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            usize::from(TRAY_QUIT),
            PCWSTR(quit.as_ptr()),
        );
        // Sans premier plan, le menu ne se ferme pas quand on clique ailleurs.
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(
            Some(hwnd),
            WM_NULL,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
        let _ = DestroyMenu(menu);
        u8::try_from(chosen.0).unwrap_or(0)
    }
}

/// Installe (une seule fois) la procédure qui reçoit les messages de la zone de notification.
#[cfg(windows)]
fn ensure_subclass(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::Shell::SetWindowSubclass;
    static SUBCLASSED: std::sync::Once = std::sync::Once::new();
    // La fenêtre appartient au fil d'interface, qui appelle cette fonction : on peut la « sous-classer » ici.
    SUBCLASSED.call_once(|| {
        // SAFETY : la procédure est une fonction statique ; la fenêtre est celle de ce processus.
        let _ = unsafe { SetWindowSubclass(hwnd, Some(tray_subclass), 1, 0) };
    });
}

/// Garde l'icône dans la zone de notification (menu « Ouvrir » / « Quitter », clic gauche = rouvrir). À appeler depuis
/// le fil d'interface ; peut être rappelée pour changer les libellés (changement de langue).
#[cfg(windows)]
pub fn tray_enable(open: &str, quit: &str) {
    use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_MESSAGE, NIM_ADD, NIM_MODIFY};
    if let Ok(mut l) = TRAY_LABELS.lock() {
        *l = (open.to_string(), quit.to_string());
    }
    let Some(mut nid) = base_icon_data() else {
        return;
    };
    ensure_subclass(nid.hWnd);
    nid.uFlags |= NIF_MESSAGE;
    nid.uCallbackMessage = TRAY_CALLBACK;
    let Ok(mut since) = TRAY_SINCE.lock() else {
        return;
    };
    PERMANENT.store(true, std::sync::atomic::Ordering::Relaxed);
    // SAFETY : `nid` est une structure complète et valide pendant l'appel.
    unsafe {
        let _ = Shell_NotifyIconW(if since.is_none() { NIM_ADD } else { NIM_MODIFY }, &nid);
    }
    *since = Some(std::time::Instant::now());
}

/// Retire l'icône permanente (option désactivée).
#[cfg(windows)]
pub fn tray_disable() {
    PERMANENT.store(false, std::sync::atomic::Ordering::Relaxed);
    remove_tray_icon();
}

/// Commande choisie dans l'icône depuis le dernier appel (`TRAY_OPEN`, `TRAY_QUIT`), sinon 0.
pub fn take_tray_command() -> u8 {
    #[cfg(windows)]
    {
        TRAY_CMD.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Notre fenêtre est-elle affichée (ni cachée dans la zone de notification) ?
#[cfg(windows)]
pub fn own_window_visible() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    // SAFETY : simple lecture sur une fenêtre de ce processus.
    own_window().is_none_or(|h| unsafe { IsWindowVisible(h) }.as_bool())
}

#[cfg(not(windows))]
pub fn own_window_visible() -> bool {
    true
}

/// Réaffiche notre fenêtre (cachée ou réduite) au premier plan.
#[cfg(windows)]
pub fn show_own_window() {
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };
    // SAFETY : appels Win32 simples sur une fenêtre de ce processus.
    unsafe {
        if let Some(hwnd) = own_window() {
            let _ = ShowWindow(hwnd, SW_SHOW);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
pub fn show_own_window() {}

#[cfg(not(windows))]
pub fn tray_enable(_open: &str, _quit: &str) {}

#[cfg(not(windows))]
pub fn tray_disable() {}

/// Procédure de la fenêtre : reçoit les messages de la zone de notification ; un clic sur la bulle mémorise la chaîne.
#[cfg(windows)]
unsafe extern "system" fn tray_subclass(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    // NIN_BALLOONUSERCLICK = WM_USER + 5 ; clics sur l'icône permanente : WM_LBUTTONUP, WM_LBUTTONDBLCLK, WM_RBUTTONUP.
    const NIN_BALLOONUSERCLICK: usize = 0x0400 + 5;
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_LBUTTONDBLCLK: usize = 0x0203;
    const WM_RBUTTONUP: usize = 0x0205;
    if msg == TRAY_CALLBACK {
        let event = lparam.0 as usize & 0xFFFF;
        if event == NIN_BALLOONUSERCLICK {
            let target = NOTIFY_TARGET.lock().ok().and_then(|t| t.clone());
            if let (Some(login), Ok(mut c)) = (target, NOTIFY_CLICKED.lock()) {
                *c = Some(login);
            }
            wake_ui();
        } else if PERMANENT.load(std::sync::atomic::Ordering::Relaxed) {
            let cmd = match event {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => TRAY_OPEN,
                WM_RBUTTONUP => tray_menu(hwnd),
                _ => 0,
            };
            if cmd != 0 {
                TRAY_CMD.store(cmd, std::sync::atomic::Ordering::Relaxed);
                wake_ui();
            }
        }
    }
    // SAFETY : transmission du message d'origine à la procédure précédente de la fenêtre.
    unsafe { windows::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Affiche une notification Windows (bulle). Sans effet si la fenêtre est introuvable. `login` : chaîne ouverte au clic
/// sur la bulle (vide : le clic ne fait rien).
#[cfg(windows)]
pub fn notify(title: &str, text: &str, login: &str) {
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_INFO, NIF_MESSAGE, NIIF_LARGE_ICON, NIIF_USER, NIM_ADD, NIM_MODIFY,
    };
    let Some(mut nid) = base_icon_data() else {
        return;
    };
    if let Ok(mut t) = NOTIFY_TARGET.lock() {
        *t = (!login.is_empty()).then(|| login.to_string());
    }
    ensure_subclass(nid.hWnd);
    nid.uFlags |= NIF_INFO | NIF_MESSAGE;
    nid.uCallbackMessage = TRAY_CALLBACK;
    // Le logo de l'application dans la bulle (au lieu du « i » bleu par défaut).
    nid.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON;
    nid.hBalloonIcon = nid.hIcon;
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, text);
    let Ok(mut since) = TRAY_SINCE.lock() else {
        return;
    };
    // SAFETY : `nid` est une structure complète et valide pendant les appels.
    unsafe {
        if since.is_none() {
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
    *since = Some(std::time::Instant::now());
    drop(since);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(12));
        let idle = TRAY_SINCE
            .lock()
            .ok()
            .is_some_and(|s| s.is_some_and(|t| t.elapsed() >= std::time::Duration::from_secs(12)));
        // L'icône permanente (option de fermeture) reste en place.
        if idle && !PERMANENT.load(std::sync::atomic::Ordering::Relaxed) {
            remove_tray_icon();
        }
    });
}

/// Retire l'icône de la zone de notification (fin de la bulle, ou fermeture de l'application).
#[cfg(windows)]
pub fn remove_tray_icon() {
    use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIM_DELETE};
    let Ok(mut since) = TRAY_SINCE.lock() else {
        return;
    };
    if since.take().is_some() {
        if let Some(nid) = base_icon_data() {
            // SAFETY : structure valide pendant l'appel.
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn notify(_title: &str, _text: &str, _login: &str) {}

#[cfg(not(windows))]
pub fn remove_tray_icon() {}

/// Langue d'affichage de Windows pour l'utilisateur (« es-ES », « zh-TW »…).
#[cfg(windows)]
pub fn user_locale() -> Option<String> {
    use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut buf = [0u16; 85];
    // SAFETY : appels Win32 sans pointeur conservé ; la mémoire tampon est locale et de taille connue.
    let n = unsafe { LCIDToLocaleName(u32::from(GetUserDefaultUILanguage()), Some(&mut buf), 0) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn user_locale() -> Option<String> {
    None
}

/// Coins arrondis de Windows 11 pour la fenêtre sans bordure (sans effet sur Windows 10).
#[cfg(windows)]
pub fn round_window_corners() {
    set_window_corners(true);
}

/// Coins arrondis de la fenêtre (Windows 11) et liseré de bordure. En plein écran : ni coins ni liseré, sinon un
/// filet clair et des coins entamés restent visibles sur les bords de l'image.
#[cfg(windows)]
pub fn set_window_corners(round: bool) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT, DWMWA_COLOR_NONE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    };
    // SAFETY : les valeurs passées vivent pendant l'appel ; la fenêtre est celle de ce processus.
    unsafe {
        if let Some(hwnd) = own_window() {
            let pref = if round {
                DWMWCP_ROUND
            } else {
                DWMWCP_DONOTROUND
            };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const pref).cast(),
                std::mem::size_of_val(&pref) as u32,
            );
            let border: u32 = if round {
                DWMWA_COLOR_DEFAULT
            } else {
                DWMWA_COLOR_NONE
            };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&raw const border).cast(),
                std::mem::size_of_val(&border) as u32,
            );
            // Pas d'ombre portée autour de la fenêtre (winit la demande pour toute fenêtre sans bordure) : marges nulles.
            if round {
                let none = windows::Win32::UI::Controls::MARGINS::default();
                let _ = windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea(hwnd, &none);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn set_window_corners(_round: bool) {}

#[cfg(not(windows))]
pub fn round_window_corners() {}

#[cfg(all(test, windows))]
mod tests_chemins_systeme {
    /// Les programmes système sont appelés par un chemin complet qui existe (pas un nom nu).
    #[test]
    fn chemins_complets_vers_le_systeme() {
        let cmd = super::system_exe("cmd.exe");
        assert!(cmd.is_absolute() && cmd.is_file(), "{}", cmd.display());
        let explorer = super::windows_exe("explorer.exe");
        assert!(
            explorer.is_absolute() && explorer.is_file(),
            "{}",
            explorer.display()
        );
    }
}

/// Boîte de message Windows bloquante (affichage impossible). `warning` : icône d'avertissement, sinon
/// d'erreur. Sert quand la fenêtre de l'application n'existe pas ou ne peut rien afficher.
#[cfg(windows)]
pub fn message_box(text: &str, warning: bool) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND,
    };
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (text, title) = (wide(text), wide("Velka"));
    let icon = if warning {
        MB_ICONWARNING
    } else {
        MB_ICONERROR
    };
    // SAFETY : chaînes terminées par un zéro, vivantes pendant l'appel ; pas de fenêtre parente.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | icon | MB_SETFOREGROUND,
        );
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn message_box(_text: &str, _warning: bool) {}

#[cfg(all(test, windows))]
mod tests_fenetre {
    use super::same_exe_name;
    use std::path::Path;

    /// Une fenêtre n'est prise que si son processus est bien `velka.exe` (même nom, casse ignorée).
    #[test]
    fn meme_programme() {
        assert!(same_exe_name(
            Path::new(r"C:\a\VELKA.exe"),
            Path::new(r"D:\b\velka.exe")
        ));
        assert!(!same_exe_name(
            Path::new(r"C:\Program Files\Google\chrome.exe"),
            Path::new(r"D:\b\velka.exe")
        ));
        assert!(!same_exe_name(Path::new(""), Path::new(r"D:\b\velka.exe")));
    }

    /// Sans fenêtre « Velka » ouverte par ce processus de test, aucune n'est trouvée (une autre fenêtre de même
    /// titre appartenant à un autre programme ne compte pas).
    #[cfg(windows)]
    #[test]
    fn aucune_fenetre_dans_les_tests() {
        assert!(super::find_window(true).is_none());
    }
}
