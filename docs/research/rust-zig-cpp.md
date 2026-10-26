# Fiche de recherche : options UI natives Rust / Zig / C++ (+ NSH, Slint)

Date d'établissement : 2026-10-25. Critères, par ordre : (1) performance (CPU/RAM/disque, démarrage à froid, fluidité sous charge), (2) couverture iOS / Android / macOS / Linux / Windows / web (wasm), (3) un seul langage, (4) interop C-ABI avec un cœur Rust/Zig.

Aucun build n'a été lancé, donc aucune mesure de performance propre n'est fournie : toute valeur chiffrée de perf citée vient d'une source et est marquée « NON VÉRIFIÉ ». Les comparaisons de perf ci-dessous sont donc qualitatives, déduites de l'architecture de rendu.

Convention de preuve : [O] officiel (README, docs, Cargo.toml du dépôt ou du crate publié) ; [C] code présent dans le dépôt, sans déclaration de support ; [E] expérimental ou pré-release déclaré ; [W] source secondaire (WebSearch).

## 1. Tableau récapitulatif

| Option | Langage(s) | Rendu | Version stable la plus récente (date) | Licence | Auteurs 12 mois (commits) | iOS | Android | macOS | Linux | Windows | Web (wasm) | C-ABI avec cœur Rust/Zig |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| NSH | Rust + TSX (QuickJS) | Vues natives (UIKit, vues Android, AppKit, GTK4, Win32/Direct2D), HTML sur le web ; GPUI en option | 0.2.0 (2026-10-22), projet de 12 jours | Apache-2.0 | 20 (408) | [O] | [O] | [O] | [O] | [O] | [O] (DOM, pas canvas) | Oui côté embarqué : `nsh.h` (iOS), JNI (Android) ; JSON à la frontière [O] |
| Slint | DSL .slint + Rust / C++ / JS / Python | Logiciel, FemtoVG (GL ou wgpu), Skia, Vello (expérimental), Qt | 1.18.1 (2026-09-21) ; main = 1.19.0 non publié | GPL-3.0-only OU Royalty-free-2.0 OU commerciale | 148 (5173) | [O] (Skia + winit) | [O] | [O] | [O] | [O] | [O] mais « non recommandé pour apps web générales » | API C++ ; pas d'API C stable documentée |
| GPUI (Zed) | Rust | GPU maison (Metal ; wgpu sur Linux/Windows/web) | gpui 0.2.2 (docs.rs : 2025-10-22 [W]) ; Zed 1.25.3 (2026-10-23) | Apache-2.0 (crate gpui) ; Zed GPL-3.0-or-later | 1211 sur tout Zed (8392) ; ~29 sur gpui_wgpu | [C] gpui_ios, premier commit 2026-10-14 | aucune preuve | [O] | [O] | [O] | [C] gpui_web (depuis 2026-02) | non |
| egui / eframe | Rust | Immédiat ; wgpu (défaut) ou glow | 0.37.0 (2026-10-15) | MIT OU Apache-2.0 | 144 (563), dont 3 au-delà de 17 commits | aucun support déclaré | [O] (eframe) | [O] | [O] | [O] | [O] (WebGPU/WebGL2) | non |
| iced | Rust | wgpu ou tiny-skia | 0.14.0 (2025-12-07) | MIT | 84 (1053) | non déclaré | non déclaré | [O] | [O] | [O] | [O] | non |
| Dioxus | Rust (RSX) | WebView (wry) ; natif Blitz expérimental | 0.7.10 (2026-07-30) ; 0.8.0-alpha.3 (2026-10-16) | MIT OU Apache-2.0 (confirmé §11) | 110 (349) | [O] WebView | [O] WebView | [O] | [O] | [O] | [O] DOM | non |
| Blitz / dioxus-native | Rust | HTML/CSS natif, Vello via wgpu | 0.2.1 stable ; 0.3.0-beta.4 (2026-10-16) | MIT OU Apache-2.0 (confirmé §11) | 37 (954), dont 860 d'un seul auteur | non déclaré | [O] « Android » dans le navigateur de démo | [O] | [O] | [O] | non | non |
| Makepad | Rust (DSL live) | Shaders GPU maison | 1.0.0 (2025-05-15) | MIT | 25 (1893), historique en grande partie sous un compte « Admin » | [O] | [O] | [O] | [O] | [O] | [O] | non |
| Xilem / Masonry / Vello | Rust | Vello (wgpu) | xilem 0.4.0 (2025-10-29) ; vello 0.11.0 (2026-10-02) | Apache-2.0 | xilem 42 (306) ; vello 35 (481) | non déclaré | non déclaré | [O] | [O] | [O] | Xilem Web (backend DOM) | non |
| Floem | Rust | wgpu (vger/Vello) ou Skia | 0.2.0 (2024-11-14) | MIT (confirmé §11) | 15 (114) | non | non | [O] | [O] | [O] | non | non |
| Freya | Rust | Skia | 0.4.3 ; 0.5.0-beta.12 (2026-10-18) | MIT | 30 (957), dont 880 d'un seul auteur | non déclaré | contribution Android mentionnée | [O] | [O] | [O] | non déclaré | non |
| Tauri (pour mémoire) | Rust + web | WebView système (wry) | 2.12.5 (2026-10-21) | MIT OU Apache-2.0 | 125 (629) | [O] (iOS 9+) | [O] (Android 8+) | [O] | [O] | [O] | n/a | commandes IPC, pas de C-ABI |
| Zig : zigui (ddalcu) | Zig | Rendu maison, SDL3 + SDL_GPU, repli logiciel | v0.3.4 (2026-07-03) ; `build.zig.zon` indique 0.2.0 | MIT | 2 (25) | non | non | [O] | [O] | [O] | non | C-ABI native de Zig |
| Zig : DVUI | Zig | Immédiat ; SDL3, SDL3GPU, SDL2, Raylib, DX11, Web | v0.5.0 (2026-10-11) | MIT | 61 (1427) | via SDL (non documenté pour DVUI) | via SDL (non documenté) | [O] | [O] | [O] | [O] backend Web | C-ABI native de Zig |
| Zig : Capy | Zig | Contrôles natifs de l'OS | v0.3.0 (2023-02-11) | MPL-2.0 (§11) | 2 (2) : inactif | non | non | non | non | non | non | |
| C++ : Dear ImGui | C++ | Immédiat ; sortie de vertices, backends au choix | v1.93.0 (2026-10-14) | MIT | 70 (868), dont 695 d'un seul auteur | [O] via SDL/Metal | [O] backend Android | [O] | [O] | [O] | [O] Emscripten, WebGPU | API C++ ; `cimgui` fournit une couche C |

« Auteurs 12 mois » = noms d'auteur distincts de `git shortlog -sn --since=12.months HEAD`, bots inclus. La méthode et ses limites sont détaillées en fin de document.

## 2. Constats transversaux sur la performance

Aucun des projets ci-dessous n'a fait l'objet d'une mesure par nous. Ce qui suit se déduit de l'architecture.

- Les deux familles de coût mémoire sont : (a) un moteur de rendu embarqué (Skia, wgpu/Vello) qui apporte des Mo de binaire et des dizaines de Mo de RAM de pilote GPU ; (b) un WebView, qui ajoute un processus de rendu complet géré par l'OS. Le WebView déplace le coût hors du binaire mais pas hors du système.
- Les options à rendu logiciel (Slint `software`, DVUI/zigui en repli, tiny-skia pour iced) évitent le GPU, mais dépensent du CPU sur les grandes surfaces.
- Le mode immédiat (egui, DVUI, ImGui) relance la construction de l'UI à chaque frame de repaint. egui ne repeint que sur événement ou demande explicite, ce qui rend l'inactivité peu coûteuse ; ImGui et DVUI exigent normalement de borner la boucle soi-même (à vérifier par mesure).
- Les options à vues natives (NSH) font payer chaque nœud comme une vue de l'OS, mais déchargent le texte, la saisie et l'accessibilité à la plateforme.

## 3. NSH (longbridge/nsh)

Sources : lecture des fichiers du clone `/home/user/eval/repos/nsh` via `git show HEAD:...` (README.md, AGENTS.md, CONTRIBUTING.md, Cargo.toml, docs/dev/architecture.md, docs/dev/performance.md, docs/dev/sandbox.md, docs/dev/embedding.md), consultés le 2026-10-25. nsh.dev et github.com sont bloqués : aucune page web n'a été lue.

### Identité, âge, version, licence
- Version de l'espace de travail : 0.2.0 (`Cargo.toml`), publiée sur l'index crates.io : `nsh-core` et `nsh-cli` ont 3 versions au total. Tags : v0.1.0 (2026-10-17), v0.1.1 (2026-10-18), v0.2.0 (2026-10-22).
- Licence : Apache-2.0 (champ `license` du `Cargo.toml` ; fichier `LICENSE-APACHE`).
- Âge : premier commit visible le 2026-10-14 (Sunli), dernier le 2026-10-26 UTC+8. Soit environ 12 jours d'historique, 408 commits. Il est possible que l'historique public ait été réécrit ou importé ; on ne peut pas le savoir sans le dépôt d'origine. Le projet est donc extrêmement jeune au regard d'un choix d'architecture.
- Édition Rust 2024 ; `rust-toolchain.toml` présent ; le crate `nsh-core` est décrit comme « le gpui de NSH ».
- Contributeurs : 20 noms distincts (20 par nom, méthode `shortlog -sn`). Répartition : Jason Lee 174, Sunli 116, Floyd Wang 43, dXn 15, Hexuria 10, Mark Watts 10, puis 8 à 1 commits. Les deux premiers font 71 % des commits. « Copilot » (6 commits) est un bot, donc 19 humains au plus. Sur 12 jours, « 12 mois » et « historique complet » coïncident.
- L'équipe est liée à Longbridge (organisation GitHub `longbridge`, qui édite aussi gpui-component ; le README cite GPUI Kit, voir sources). Le fichier AGENTS.md et CLAUDE.md montrent un développement fortement assisté par agents IA.

### Architecture réelle
- Trois couches [O] : un runtime JS (QuickJS, dans `nsh-js`), un cœur indépendant de la plateforme (`nsh-core` : arbre d'ombre, styles « RN semantics », layout Taffy flex/grid/block, graphe réactif à la Solid 1.9, listes virtualisées, animations natives) et des backends par plateforme (`nsh-platform-*`).
- Threads : le JS tourne sur un thread moteur (engine thread) ; chaque commit produit un `MountBatch` par frame, transmis au thread principal par un seul saut de file. « Le thread principal de la plateforme n'attend jamais le JS », avec une exception bornée sur macOS : attente jusqu'à 16 ms pour le layout lors d'un redimensionnement (`docs/dev/architecture.md`).
- Le texte est mesuré et disposé une seule fois sur le thread moteur (CoreText, StaticLayout, Pango, DirectWrite, système de texte de GPUI), puis dessiné tel quel sur le thread principal.
- Rendu par plateforme [O, docs/dev/architecture.md] : iOS = une UIView par nœud ; Android = vues Android, un `byte[]` par commit vers `Mounter.java` ; macOS = une NSView par nœud avec Core Animation ; Windows = un arbre retenu dessiné en Direct2D dans une swap chain ; Linux = un widget GTK4 par nœud, nœuds GSK ; web = DOM et CSS, moteur sur le thread principal de la page ; GPUI (macOS/Windows/Linux) = un élément GPUI par surface.
- Runtime TSX/JS : QuickJS exécute le code TSX de l'application, avec un compilateur JSX maison et un bundler (rolldown + Oxc). Le moteur est un QuickJS par surface, sans singletons globaux, donc intégrable plusieurs fois dans une même application. La chaîne d'outils est du Rust pur, sans Node.js.
- Mode Rust seul : un exemple `gallery-rs` est décrit comme « une application sans TSX et sans QuickJS ». Les composants Rust s'exécutent une fois (modèle à signaux). Il est donc possible de n'utiliser que Rust, ce qui est pertinent pour votre critère « un seul langage ».
- Sandbox [O, `docs/dev/sandbox.md`] : une VM QuickJS par surface ; en intégration, l'hôte fixe les limites (par défaut 256 Mo de tas, 4 Mo de pile, 10 s de chargement, 2 s par appel, 200 000 nœuds) ; durcissement (`eval` supprimé, constructeurs `Function` qui lèvent, prototypes gelés) ; capacités déclarées (`network`, `fs`, `process`, `clipboard`, `webview`…) ; paquets signés Ed25519 (`.nshpkg`). Une application autonome n'a aucune limite de tas ni de temps.
- FFI [O, `docs/dev/embedding.md`] : iOS expose une C-ABI (`include/nsh.h`, symboles `_nsh_*`, module Swift `NativeShell`) ; Android passe par JNI (`NshView`) ; les données traversent la frontière sous forme de JSON. Il n'y a pas de C-ABI générique documentée pour un cœur Rust/Zig externe : la liaison attendue se fait en Rust (crates `nsh-core` + `#[native_module]`) ou via un hôte natif.
- Wasm : `nsh-platform-web` produit `nsh.wasm` ; l'application publiée utilise un runtime sans parseur QuickJS (feature `bytecode-only`). `docs/dev/web.md` indique qu'une réduction de la charge de première visite est passée de 676 Ko (195 Ko brotli) à 91 Ko (24 Ko) pour la part de l'application : NON VÉRIFIÉ (source docs/dev/web.md du dépôt NSH, consulté 2026-10-25), et ce chiffre exclut le wasm du moteur.

### Maturité par plateforme
Le README déclare toutes les plateformes ; aucune ne porte d'étiquette « alpha » dans README ni docs relues. Les exigences : iOS 15+, Android 9 (API 28)+, macOS 14+, Windows 10 1809+, GTK 4.14+, navigateurs modernes. Preuve : déclaratif [O] pour toutes, avec un harnais de tests de compatibilité (`tests/compat`, `nsh test --target`). Mais 12 jours d'âge et une version 0.2.0 signifient qu'aucune application tierce en production n'est documentée. Aucun retour d'usage externe n'a été vérifié.

### Performance
- Le projet affirme « Performance is the project's first priority » (`docs/dev/performance.md`). Il publie des lignes de base mesurées par ses auteurs, par exemple : démarrage de l'hôte `Host::new` ~75 à 155 µs en release ; premier frame du compteur ~1,05 à 1,15 ms sur Catalyst (release) ; défilement de liste : commit médian ~0,26 à 0,31 ms ; Android émulateur arm64 : premier frame de la galerie 20 à 50 ms. Tous NON VÉRIFIÉS (source docs/dev/performance.md de NSH, consulté 2026-10-25) : mesures d'auteurs, souvent sur machines bruitées, plusieurs sont en « headless » (sans fenêtre réelle) ou en debug, et l'auteur note lui-même que les mesures sur une seule frame sont dominées par l'état d'horloge du CPU.
- Aucune mesure de RAM ni de taille de binaire n'a été trouvée dans les fichiers lus. Le coût d'un QuickJS par surface et d'une vue native par nœud n'est donc pas quantifié.
- Contraintes structurelles à garder en tête : chaque nœud est une vue native (coût à l'échelle de milliers de nœuds, atténué par la virtualisation de listes) ; le GC de QuickJS parcourt toute la VM (30 à 90 ms avec une grosse démo, selon le document de perf, NON VÉRIFIÉ) et est déclenché après 0,3 s d'inactivité.

## 4. Slint

Sources : clone `/home/user/eval/repos/slint` (README.md, LICENSE.md, Cargo.toml, CHANGELOG.md, docs/astro/src/content/docs/guide/{platforms,backends-and-renderers}/*), consultés le 2026-10-25. slint.dev bloqué : docs lues dans le dépôt.

- Version publiée la plus récente : 1.18.1 (tag 2026-09-21, aussi l'index crates.io) ; 1.18.0 le 2026-09-16 ; 1.17.1 le 2026-07-06. `main` porte la version 1.19.0, marquée « Unreleased » dans le CHANGELOG. `rust-version = "1.95"`.
- Licence [O, LICENSE.md, Cargo.toml] : `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`. Le Royalty-free permet les applications propriétaires de bureau, mobile et web gratuitement, mais exclut l'embarqué ; la GPLv3 couvre l'open source, embarqué compris ; la licence commerciale couvre tout, prix sur slint.dev/pricing (non lu, hôte bloqué). La documentation et les exemples sont sous MIT. Le texte exact de la licence Royalty-free 2.0 (conditions d'attribution notamment) n'a pas été relu ici : à lire dans `LICENSES/LicenseRef-Slint-Royalty-free-2.0.md` avant décision.
- Contributeurs : 148 noms distincts sur 12 mois (152 adresses ; ~146 hors bots), 5173 commits, dernier commit 2026-10-25. 33 auteurs avec au moins 5 commits ; 67 auteurs sur 3 mois (hors bots). Plus actifs : Simon Hausmann 1460, Olivier Goffart 1346, Leon Matthes 351, David Faure 323, Nigel Breslaw 253. dependabot[bot] : 133 commits. L'activité est portée par l'entreprise SixtyFPS/Slint.
- Architecture : un compilateur du DSL `.slint` vers du code natif (Rust/C++) ou un interpréteur (JS, Python, outils). Le README dit que composants, éléments et propriétés sont disposés « dans une seule région mémoire » pour réduire les allocations (affirmation de conception, non mesurée ici). Réactivité par liaisons de propriétés. Un seul thread d'UI (modèle classique).
- Backends [O] : `winit` (défaut), `qt`, `linuxkms` (« encore expérimental », sans compositeur), plus plateforme personnalisée. Moteurs de rendu [O] : `software` (CPU, rendu partiel, ligne par ligne en Rust, adapté aux microcontrôleurs, limites : pas de rotation ni d'échelle, texte limité aux écritures occidentales), `femtovg` (OpenGL ES 2.0, ou wgpu), `skia` (OpenGL/Metal/Vulkan/D3D, « empreinte disque lourde »), `vello` (expérimental, jamais sélectionné automatiquement, exige des compute shaders), `qt` (QPainter).
- iOS [O] : « les applications Slint en Rust ou C++ peuvent être compilées pour iOS », via winit + Skia (le Skia OpenGL n'est pas pris en charge sur iOS). Android [O] : Rust et C++ via `android-activity` ; backend choisi par CMake pour C++. Web [O] : Rust compilé en wasm via winit + FemtoVG sur un `<canvas>` WebGL ; C++ via Emscripten. La documentation précise que Slint ne rend pas le DOM : pas de recherche, pas d'accessibilité (lecteurs d'écran indisponibles) et conclut que l'exécution dans le navigateur n'est « actuellement pas recommandée pour des applications web généralistes ».
- Langages [O] : Rust, C++, JavaScript/Node, Python (README, docs). Interop : l'API C++ est publique ; aucun C ABI documenté pour l'application. Pour un cœur Zig, il faudrait passer par l'API C++ (ou par le cœur Rust).
- Performance : pas de benchmark officiel relu. La recherche web n'a trouvé aucun chiffre de RAM ou de taille de binaire pour Slint ; aucun chiffre n'est donc cité. Le positionnement « léger » (README) et l'existence d'un rendu logiciel et d'un mode `no_std` sont des déclarations de conception. Le CHANGELOG 1.19 mentionne « Faster startup: system fonts are discovered in the background » et « Improved the performance of layouts with wrapping text ».

## 5. Rust pur

### 5.1 GPUI (Zed)
Sources : clone `/home/user/eval/repos/gpui-zed` (README de `crates/gpui`, Cargo.toml, liste de `crates/`), consulté 2026-10-25 ; docs.rs via WebSearch uniquement (hôte bloqué).
- Hybride mode immédiat/retenu, accéléré GPU ; « toujours pré-1.0, changements cassants fréquents » (README gpui) [O].
- Crate `gpui` 0.2.2, Apache-2.0 ; date 2025-10-22 relevée par WebSearch sur docs.rs [W], non recoupée avec l'index (qui n'a pas de date). Le dépôt `main` contient des crates `gpui_macos`, `gpui_windows`, `gpui_linux`, `gpui_wgpu`, `gpui_platform`, `gpui_web` (premier commit 2026-02-26) et `gpui_ios` (premier commit 2026-10-14, 3 auteurs sur 12 mois). Aucune ligne n'indique de support officiel pour iOS ou le web : ces crates sont du code présent [C], pas une promesse. Rien pour Android n'a été trouvé.
- Zed (l'éditeur, GPL-3.0-or-later) : tag stable v1.25.3 (2026-10-23). 1211 auteurs distincts et 8392 commits sur 12 mois concernent tout le dépôt Zed, pas GPUI seul. Pour `gpui_wgpu` : 29 auteurs distincts sur 12 mois.
- NSH s'appuie sur GPUI comme backend de bureau/intégration ; il l'emploie dans un espace de travail séparé.
- Performance : GPU natif sur Metal ; wgpu sur Linux/Windows/web. Aucun chiffre vérifié.

### 5.2 egui / eframe
Sources : clone `egui`, README et `crates/eframe/README.md`, 2026-10-25.
- Immédiat, très portable ; eframe cible Web, Linux, macOS, Windows, Android [O]. iOS : pas de déclaration. Version 0.37.0 (2026-10-15), MIT OU Apache-2.0. 144 auteurs/12 mois, mais 2 auteurs font l'essentiel des commits (Emil Ernerfeldt 260, Lucas Meurer 88) ; 9 auteurs ≥5 commits. Dernier commit 2026-10-21.
- Wasm [O] : WebGPU ou WebGL2 sur un canvas ; limites déclarées : pas de recherche dans la page, accessibilité seulement expérimentale, édition de texte mobile moins bonne ; « usage suggéré : applications web où performance et réactivité priment sur l'accessibilité ».
- Rendu : wgpu par défaut, glow en option. Rustc suffit ; pas de runtime/GC. FFI : pas de C-ABI officielle.

### 5.3 iced
- Version 0.14.0 (2025-12-07) ; `main` = 0.15.0-dev. MIT. README : « Iced est actuellement un logiciel expérimental ». Cible déclarée : Windows, macOS, Linux, Web. Pas de mention de mobile dans le README relu. 84 auteurs/12 mois, 1053 commits, dont 704 + 193 (deux identités du même mainteneur, Héctor Ramón Jiménez / Héctor) : 7 auteurs ≥5 commits. Moteurs : wgpu et tiny-skia (rendu logiciel).

### 5.4 Dioxus et Blitz
- Dioxus 0.7.10 (2026-07-30), pré-version 0.8.0-alpha.3 (2026-10-16). Le rendu bureau et mobile passe par un WebView (`wry` + `tao`, vu dans `packages/desktop/Cargo.toml`). Le README annonce « support de première classe Android/iOS » (via WebView) et un « moteur de rendu natif expérimental » (Blitz, wgpu). Web : DOM via wasm. 110 auteurs/12 mois, 349 commits, 4 auteurs ≥5 commits ; dernier commit 2026-10-25.
- Blitz : « beta », exécutable sous forme de navigateur de démo sur Windows/macOS/Linux/Android (blitz.is/downloads, non consulté ; mention du README). 0.3.0-beta.4 (2026-10-16). 37 auteurs, 954 commits dont 860 de Nico Burns : dépendance à une personne. Rendu : HTML/CSS natif avec Vello ; sans JavaScript.

### 5.5 Makepad
- Version 1.0.0 (2025-05-15), tag `1.0.0`. Licence MIT (fichier `LICENSE`, Makepad B.V.). Le README annonce natif et web (wasm), avec chaîne `cargo makepad` pour iOS, tvOS, Android et wasm ; mentionne aussi Quest/XR. Options de réduction de taille wasm (`--wasm-opt`, `--brotli`, `--split`) : bon signe pour le critère disque, aucun chiffre relevé. 25 auteurs/12 mois, 1893 commits, dont environ 1464 sous le compte générique « Admin » : l'attribution individuelle est donc illisible. Le projet se présente comme « accéléré par l'IA » et DSL Splash.

### 5.6 Xilem / Masonry / Vello
- README Xilem : « architecture expérimentale ». xilem 0.4.0 (2025-10-29), Apache-2.0. Backends : Masonry (natif) et web (DOM). vello 0.11.0 (2026-10-02). 42 auteurs/12 mois pour xilem, 35 pour vello. Pas de support mobile déclaré. Dernier commit xilem : 2026-09-14.

### 5.7 Floem, Freya, Tauri
- Floem : v0.2.0 (2024-11-14), MIT présumé, 15 auteurs/12 mois, 114 commits, un seul auteur actif sur 3 mois. Rendu wgpu (vger/Vello) ou Skia ; Windows, macOS, Linux seulement [O].
- Freya : 0.4.3 stable, 0.5.0-beta.12 (2026-10-18), MIT ; rendu Skia ; mention d'une contribution Android ; 880 des 957 commits d'un seul auteur.
- Tauri 2.12.5 (2026-10-21) : WebView système via wry ; iOS 9+, Android 7+ (actuellement 8+), toutes les plateformes de bureau [O]. 125 auteurs/12 mois (dont renovate[bot]). Pas de moteur de rendu propre : coût RAM/CPU du WebView de l'OS.

## 6. Zig

Sources : clones `zigui-ddalcu`, `dvui`, `capy`, `zgui`, consultés 2026-10-25 ; ziglang.org bloqué ; codeberg (dépôt de Zig) inaccessible via proxy (403).
- Zig lui-même : le `build.zig.zon` de zigui exige `minimum_zig_version = "0.17.0"` et DVUI est « testé avec Zig 0.17.0 » ; des blogs [W] décrivent Zig 0.17.0 (build system réécrit, `@cImport` retiré d'après un blog coréen non recoupé). Date de sortie officielle non vérifiée.
- zigui (github.com/ddalcu/zigui) : « pré-alpha mais boîte à outils utilisable » ; Zig pur, SwiftUI-like, rendu maison identique partout via SDL_GPU (Metal, Vulkan) avec repli logiciel ; SDL3 pour la fenêtre. macOS, Linux, Windows seulement. MIT. Tags jusqu'à v0.3.4 (2026-07-03) mais le fichier `.zon` dit 0.2.0 : incohérence. 2 auteurs (un seul compte d'e-mail), 25 commits sur 12 mois. Il existe un fichier CLAUDE.md : développement assisté par IA. Maturité : projet personnel.
- DVUI (david-vanderson/dvui) : immédiat, MIT. v0.5.0 (2026-10-11), tags précédents 0.4.0 (2026-04-16), 0.3.0 (2025-08-30). Backends : SDL3, SDL3GPU, SDL2, Raylib, DX11, Web (wasm). 61 auteurs/12 mois (69 e-mails), 1427 commits dont 785 du mainteneur ; 26 auteurs ≥5 commits : communauté réelle, la plus saine des options Zig. Mobile : pas de déclaration DVUI (dépend de SDL).
- Capy : dernier commit 2025-12-25, 2 commits en 12 mois : à considérer comme inactif ; cible Zig 0.14.1 selon un résultat de recherche [W].
- Zig + SDL3 : SDL (libsdl-org/SDL) : 268 auteurs/12 mois, 2152 commits, release 3.4.18 (2026-10-02), couvre iOS/Android/macOS/Linux/Windows et Emscripten ; mais fournit fenêtre/entrée/GPU, pas de widgets. Licence zlib (habituelle pour SDL, non relue dans ce dépôt).
- zgui (zig-gamedev) : liaisons Dear ImGui pour Zig, 27 commits/12 mois, 16 auteurs.
- Zig comme cœur C-ABI : Zig exporte nativement des symboles `export fn` en convention C et importe des en-têtes C sans couche d'enrobage. C'est un argument de conception du langage connu ; ici il n'a pas été exercé.

## 7. C++ : Dear ImGui et autres
- Dear ImGui v1.93.0 (2026-10-14), MIT. Immédiat, produit des tampons de vertices rendus par un backend au choix : Win32, SDL2/SDL3, GLFW, Android natif, Metal, Vulkan, DX9–12, WebGPU, SDL_GPU ; Emscripten pris en charge [O, docs/BACKENDS.md]. 70 auteurs/12 mois, 868 commits dont 695 du mainteneur Omar Cornut ; 7 auteurs ≥5 commits. Conçu pour outils et débogage : accessibilité et texte complexe (IME, bidi) limités (connu, non vérifié ici). Interop C : `cimgui` (dernier commit 2026-10-18, 4 auteurs) génère une API C.
- Autres pistes C++ non évaluées en profondeur : Qt (le clone `qtbase` existe dans le dossier des dépôts mais n'a pas été analysé ici), Avalonia, Flutter, Compose Multiplatform, React Native, Lynx, Skip, Swift (clones présents, hors périmètre de cette fiche).

## 8. Méthode de comptage des contributeurs et limites
- Commande : `git clone --filter=blob:none --no-checkout <url> /home/user/eval/repos/<nom>` (clone anonyme), puis `git shortlog -sn --since=12.months HEAD`, `git log --since=12.months --format=%ae | sort -u`, `git rev-list --count --since=12.months HEAD`, `git tag --sort=-creatordate --format='%(refname:short) %(creatordate:short)'`. Script : `/tmp/claude-0/-home-user/950b1927-9cd8-5fd1-8b28-a00f39a279fc/scratchpad/stats.sh`.
- Fenêtre « 12 mois » = depuis 2025-10-25 (date locale) sur la branche par défaut clonée.
- « Actif » : trois lectures ont été calculées, à ne pas mélanger : noms distincts (toutes personnes), e-mails distincts hors motifs de bots, noms avec au moins 5 commits. Un auteur d'un seul commit compte dans la première lecture et compte rarement comme « actif » au sens d'une équipe.
- Limites : (a) un même humain peut avoir plusieurs noms ou e-mails (ex. iced : « Héctor Ramón Jiménez » et « Héctor » ; zigui : « ddalcu » et « David Dalcu ») ; (b) les bots (dependabot, renovate, Copilot, « SDL Wiki Bot ») sont partiellement filtrés par motif de nom, donc imparfaitement ; (c) les squashs de PR attribuent le commit au fusionneur ou à l'auteur selon le dépôt ; (d) Makepad concentre l'historique sous « Admin », et NSH a moins de 2 semaines d'historique ; (e) pour Zed, le décompte porte sur tout le monorepo (éditeur, extensions), pas sur GPUI ; (f) les tags donnent la date du tag, pas forcément celle de publication sur crates.io, dont l'index public n'expose pas de date.

## 9. Sources (URL, date de consultation 2026-10-25 sauf mention)

Lecture directe de clones git locaux (méthode de remplacement car github.com est bloqué) :
- https://github.com/longbridge/nsh (clone `/home/user/eval/repos/nsh`, HEAD 6a7b56f8, 2026-10-26)
- https://github.com/slint-ui/slint (clone, HEAD 592a25e3, 2026-10-25)
- https://github.com/zed-industries/zed ; https://github.com/emilk/egui ; https://github.com/iced-rs/iced ; https://github.com/DioxusLabs/dioxus ; https://github.com/DioxusLabs/blitz ; https://github.com/makepad/makepad ; https://github.com/linebender/xilem ; https://github.com/linebender/vello ; https://github.com/lapce/floem ; https://github.com/marc2332/freya ; https://github.com/tauri-apps/tauri
- https://github.com/ddalcu/zigui ; https://github.com/david-vanderson/dvui ; https://github.com/capy-ui/capy ; https://github.com/zig-gamedev/zgui ; https://github.com/libsdl-org/SDL ; https://github.com/ocornut/imgui ; https://github.com/cimgui/cimgui
Index des crates (versions, non datées) :
- https://index.crates.io/ (chemins slint, gpui, egui, eframe, iced, dioxus, makepad-widgets, xilem, masonry, vello, floem, freya, tauri, dioxus-native, blitz, nsh-core, nsh-cli, gpui-component)
Recherches web (WebSearch) :
- https://github.com/ddalcu/zigui (résumé de recherche)
- https://docs.rs/crate/gpui/0.2.2 (date 2025-10-22 relevée dans un résultat de recherche ; non ouvert)
- https://linuxiac.com/?p=220546 et https://braindetox.kr/posts/zig_0_17_0_release_build_system_2026.html (Zig 0.17, sources secondaires, non recoupées)
- https://nyc1.lr.ggtyler.dev/r/rust/comments/1jzawxo/small_footprint_gui_library (anecdotique, non cité comme fait)
Non accessibles ou non lus : nsh.dev, slint.dev, docs.rs (ouverture), ziglang.org, codeberg.org/ziglang/zig (403).

## 10. Points non vérifiés
- Aucune mesure de CPU, RAM, disque, démarrage à froid ou fluidité n'a été faite (consigne : aucun build). Tous les chiffres de performance de NSH viennent des fichiers `docs/dev/performance.md` et `docs/dev/web.md` de NSH et sont NON VÉRIFIÉS.
- Taille de binaire et RAM de Slint, NSH, GPUI, egui, Makepad, DVUI : non trouvées ou non mesurées.
- Maturité réelle d'iOS pour Slint (Skia + winit) et d'Android pour NSH : documents déclaratifs seulement, rien n'a été exécuté. Idem pour toutes les cases « [O] ».
- NSH : l'historique public commence le 2026-10-14 ; on ignore si le dépôt a été importé après un développement antérieur. Aucune adoption externe vérifiée. Les prix et conditions exactes de la licence Royalty-free 2.0 de Slint ne sont pas relus ici.
- GPUI : support iOS, web et Android non officiel ; la date de publication de gpui 0.2.2 vient d'un résultat de recherche docs.rs, non recoupé.
- Licences non relues dans les fichiers : Dioxus (MIT OU Apache-2.0 d'après la pratique du dépôt, non vérifié), Floem, Blitz, SDL (zlib présumé), Capy.
- Zig 0.17.0 : sortie confirmée seulement par sources secondaires et par `minimum_zig_version` de projets ; notes officielles non lues (ziglang.org bloqué).
- Interop C-ABI : seul NSH (iOS `nsh.h`, JNI) et Dear ImGui (cimgui) ont une interface C documentée ; pour Slint, la présence d'une API C stable n'a pas été établie.
- Pas d'évaluation de Qt, Flutter, Compose Multiplatform, Avalonia, React Native, Lynx, Skip, Swift : clones présents mais hors du périmètre demandé.
- Les comptages d'auteurs bruts incluent des bots et des doublons d'identité (voir §8).

## 11. Mises à jour après réouverture des hôtes (consultées le 2026-10-25)

Hôtes lus : nsh.dev, slint.dev, ziglang.org, docs.rs (HTTP 200). crates.io (web/API) reste bloqué (403). Aucun build, aucune mesure propre.

### NSH (https://nsh.dev/llms.txt, /docs/platforms.md, /docs/comparison.md, /docs/performance-report.md)
- CONFIRMÉ : plateformes et prérequis du README (iOS 15+, Android 9+ avec arm64-v8a/armeabi-v7a/x86_64, macOS 14+, Windows 10 1809+, GTK 4.14+, GPUI sur macOS/Windows/Linux Vulkan, web HTML). Le mode Rust sans QuickJS est confirmé (« no JavaScript VM »). Les limites de la ligne « embed » sont précisées : l'`AppView` macOS/Windows/Linux charge des bundles locaux avec les limites par défaut ; le SDK iOS/Android et GPUI chargent aussi des paquets signés et fixent limites et capacités par vue.
- NOUVEAU, chiffres du site (auto-mesurés par NSH, donc NON VÉRIFIÉS par nous ; machine Linux x86_64 24 threads, GPU NVIDIA, écran 120/144 Hz) : galerie à 142-144 FPS, frame p95 de 0,5 à 4,2 ms, CPU de 1 à 8 % en défilement, 25-32 % pour la démo Stocks animée. Mémoire au repos (fenêtre 480x640, release) : app Rust ~49 Mo privés / ~142 Mo RSS avec le rendu GL, ~28 Mo / ~95 Mo avec Cairo ; compteur TSX ~79 Mo / ~166 Mo ; une fenêtre GTK4 avec un label sert de référence (~62 / ~132 Mo en Vulkan). Taille du runner : hello Rust 16,5 Mo (archive 7,5 Mo), hello TSX 25,7 Mo (11,6 Mo), apps réelles 20,9 à 22,3 Mo. Ces chiffres comblent le trou signalé au §3 (RAM et binaire), mais restent des déclarations du projet.
- INFIRMÉ en partie : le §3 disait qu'aucune chiffre de RAM/binaire n'existait ; ils existent sur le site. Les chiffres iOS/Windows/macOS de la page « performance report » n'ont pas été lus (troncature de la sortie).
- Auto-comparaison du site : NSH se donne « Yes » iOS/Android/web, et classe GPUI (iOS, Android, web : « No ») et Slint (iOS « Partial, Rust ») sans mesure de perf (« les marques décrivent la disponibilité, pas la performance »). Elle confirme aussi la lecture §5.1 : GPUI sans mobile ni web côté produit stable.
- TOUJOURS NON VÉRIFIÉ : aucune page de maturité par plateforme (alpha/bêta) n'a été trouvée dans llms.txt ; adoption externe ; C-ABI générique (la table d'embedding du site confirme Swift/Java/Rust seulement, pas de C-ABI pour cœur Zig).

### Slint (https://slint.dev/, licence dans le clone)
- CONFIRMÉ et précisé, Royalty-free 2.0 (SixtyFPS GmbH) : usage gratuit pour applications bureau, mobile et web ; condition d'attribution obligatoire (widget `AboutSlint` dans un écran « À propos », ou badge « Made with Slint » sur une page publique) ; interdit : l'embarqué, la redistribution du logiciel seul, et la distribution d'une application qui expose tout ou partie des API de Slint. Les autres licences : GPLv3 et commerciale (grille tarifaire non lue : pricing page non analysée en détail).
- slint.dev annonce « le runtime Slint tient dans moins de 300 Kio de RAM » : NON VÉRIFIÉ (marketing, slint.dev, 2026-10-25) ; vise le microcontrôleur avec le rendu logiciel, pas le bureau. Aucun chiffre bureau/mobile officiel trouvé.
- iOS/Android/wasm : l'état du §4 (docs du dépôt) n'a pas été contredit ; la page web de slint.dev n'a pas été relue ligne à ligne.

### Zig (https://ziglang.org/download/index.json, /download/0.17.0/release-notes.html)
- CONFIRMÉ : Zig 0.17.0 publié le 2026-10-01 (0.16.0 le 2026-04-13, 0.15.2 le 2025-10-11 ; `master` daté 2026-10-24). Les notes confirment : `@cImport` déprécié en 0.16.0 et retiré en 0.17.0, `std.Build.Step.TranslateC` déprécié au profit d'un paquet externe `translate-c` (ZSF). Conséquence pour une UI Zig utilisant des en-têtes C (SDL3, ImGui) : passer par le paquet translate-c ou des liaisons. Le blog coréen cité au §6 est donc corroboré sur ce point.
- Écosystème UI : aucune section des notes ne traite des UI ; l'état reste celui du §6 (DVUI actif, zigui personnel, Capy inactif). zigui exige 0.17.0 (donc possible uniquement depuis 2026-10-01 pour une version stable).

### GPUI (https://docs.rs/crate/gpui/latest, https://docs.rs/releases/search?query=gpui)
- CONFIRMÉ : dernière version publiée `gpui` 0.2.2, « Zed's GPU-accelerated UI framework ». DATE NON TRANCHÉE : la recherche docs.rs affiche « Oct 16, 2026 » pour gpui-0.2.2 (probablement une reconstruction de doc), alors qu'un résultat de WebSearch affichait « 2025-10-22 ». La date de première publication n'est pas établie ; crates.io (403) aurait tranché. À traiter comme non vérifiée.
- Écosystème tiers visible sur docs.rs : `gpui-ce`, `gpui-base`, `gpui-component` (existent, non évalués).

### Licences « à confirmer » (dépôts clonés, 2026-10-25)
- Dioxus : `MIT OR Apache-2.0` (Cargo.toml + LICENSE-MIT/LICENSE-APACHE) : CONFIRMÉ.
- Blitz : `MIT OR Apache-2.0` : CONFIRMÉ.
- Floem : `MIT` (fichier LICENSE) : CONFIRMÉ.
- SDL : licence zlib (texte « provided as-is… », Copyright 1997-2026 Sam Lantinga) : CONFIRMÉ.
- Capy : Mozilla Public License 2.0 (fichier LICENSE) : CONFIRMÉ.
- Freya : MIT (README) ; Makepad : MIT (LICENSE) ; Xilem : Apache-2.0 (README) : inchangés.

### Reste non vérifié
Perf mesurée par nous (aucune) ; RAM/binaire de Slint desktop, GPUI, egui, Makepad, DVUI ; date de première publication de gpui 0.2.2 ; maturité par plateforme de NSH au-delà des déclarations ; prix Slint ; chiffres de la page performance de NSH pour iOS, macOS et Windows.
