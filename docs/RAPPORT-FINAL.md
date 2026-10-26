# Banc comparatif de stacks UI natives multiplateformes — rapport final

> ## ⚠️ État du dépôt : mesures faites avec des versions en partie NON à jour
> Un audit des versions (2026-10-26) montre que plusieurs outils utilisés n'étaient pas les plus récents. **Les chiffres ci-dessous concernant Slint, egui, NSH, Avalonia, Qt et Compose, ainsi que les deux portes F\*, doivent être considérés comme provisoires** : ils sont remesurés avec les dernières versions (voir §0). Flutter 3.47.8, Slint 1.18.1, eframe 0.37.0, Avalonia 12.1.4, Compose 1.12.2 (stable) et KaRaMeL (HEAD 98ffca5) étaient déjà les dernières versions.

## 0. Audit des versions (vérifié le 2026-10-26)

| Élément | Utilisé dans les mesures ci-dessous | Dernier disponible (source) | Statut |
|---|---|---|---|
| F\* | 2025.03.25 (Z3 4.13.3) | **v2026.10.25** (tags du dépôt FStarLang/FStar) | à refaire (installé dans `/opt`, pas encore rejoué) |
| KaRaMeL | 98ffca5 | HEAD 98ffca5 (`git ls-remote`) | à jour |
| SDK .NET | 10.0.113 (paquet Ubuntu) | **10.0.403** (`releases.json` Microsoft) | à refaire (installé) |
| Rust | 1.97.0 (Slint, egui) | **1.99.1** (`channel-rust-stable.toml`) | à refaire (installé) |
| Slint / eframe | 1.18.1 / 0.37.0 | 1.18.1 / 0.37.0 (index crates.io) | à jour |
| Avalonia | 12.1.4 | 12.1.4 (NuGet) | à jour (FSharp.Core lié au SDK) |
| Compose Multiplatform | 1.12.2 | 1.12.2 stable (1.13.0-beta01 = bêta) | à jour ; Kotlin 2.4.21 (2.5.0-Beta2 = bêta) |
| Gradle | 8.14.3 | **9.8.2** (`services.gradle.org`) | à refaire (installé) |
| JDK | 21 (FFM en preview) | **25** (paquet Ubuntu `openjdk-25`) | à refaire (installé) |
| Qt | 6.4.2 (paquet Ubuntu) | **6.12.0 LTS** (`download.qt.io`) | à refaire — l'installation via `aqtinstall` s'est arrêtée faute d'espace disque (`Insufficient disk space`, `logs` dans `toolchains-logs/qt612.log`) |
| Flutter / Dart | 3.47.8 / 3.13.7 | 3.47.8 stable (`releases_linux.json`) | à jour |
| NSH | commit 6a7b56f847 (2026-10-25) | HEAD 5757911cf2 (tag v0.2.0 inchangé) | à mettre à jour |
| Mesa (llvmpipe) | 25.2.8 (Ubuntu noble-updates) | non vérifié en amont | dépendance système, limite connue |


Date des mesures : 2026-10-25/26. Machine : VM Linux x86_64, 4 vCPU, 16 Go, **sans GPU**, rendu logiciel sous Xvfb 1280×720. Tout ce qui est donné comme mesuré a été exécuté ; les logs et JSON bruts sont dans ce dépôt (`results/`, `logs/`, `stacks/*/logs/`).

## 1. Résumé

- **Ce qui est mesuré** : le coût CPU/RAM/latence de rendu *logiciel* de 8 stacks sur Linux, pour une app de référence identique (liste virtualisée de 100 000 lignes à hauteur variable, flux de mises à jour, canvas animé, texte de 1 Mo, images, navigation, appels C-ABI, stress). **Ce n'est pas** une mesure de fluidité sur GPU, sur iOS/Android ni sur wasm.
- **Verdict perf (rendu logiciel, Linux)** : Qt et Slint sont nettement devant sur le défilement et les mises à jour en flux ; Slint a la plus faible empreinte RAM (23 Mo idle) et reste le plus fluide en stress (≈41 FPS contre 14 pour Qt et ≤10 pour les autres). Avalonia en NativeAOT est très léger (40 Mo idle) et démarre en 118 ms. Compose (JVM) et Flutter sont les plus lourds (RAM idle 168 et 243 Mo, démarrage 1,7 s et 0,44 s).
- **Le canvas vectoriel (5 000 formes avec dégradés/ombres/clip) est le point faible de toutes les stacks en rendu logiciel** (de 0,3 à 18 FPS). Ce résultat est dû au scénario choisi, très exigeant, et à l'absence de GPU ; il ne dit pas comment ces stacks se comportent avec un GPU.
- **Portes F\*** : F\* → F# → NativeAOT = *utilisable avec contournements* ; F\* (Low\*) → C via KaRaMeL → FFI = *utilisable* (construction reproductible, 209 lignes de F\* pour 57 lignes de C).
- **Classement conditionnel et recommandations** : section 8. **Plan de migration depuis KMP** : section 9 (proposition, non testée sur mobile).

> **Correction** : un résumé intermédiaire donné en cours de session affichait pour Compose des valeurs de l'ancienne version 1.5.12 (list 144 FPS, canvas 1,9, RAM idle 123 Mo) par erreur d'agrégat. Les tableaux ci-dessous sont générés par `harness/synthese.py` depuis les JSON bruts et donnent pour **Compose 1.12.2** : list 225 FPS, canvas 4,7 FPS, RAM idle 168 Mo. Les données de la 1.5.12 sont conservées sous `results/compose-cmp1.5.12-*`.

## 2. Périmètre : ce qui a été retenu, écarté, et pourquoi

| Stack | Statut | Remarque |
|---|---|---|
| Slint (Rust + DSL `.slint`, rendu logiciel) | mesurée | variante FemtoVG/llvmpipe en secondaire |
| Qt 6.4.2 (C++/QML) | mesurée | version Ubuntu, bien plus ancienne que la LTS 6.12 (limite de comparabilité) ; variante OpenGL/llvmpipe en secondaire |
| Avalonia 12.1.4 (F#, .NET 10) | mesurée en NativeAOT **et** JIT | UI en F# pur, sans XAML ni FuncUI |
| Compose Multiplatform 1.12.2 (Kotlin, desktop JVM) | mesurée | baseline KMP ; 1.5.12 mesurée aussi (voir §4) |
| Flutter 3.47.8 (Dart AOT) | mesurée | Impeller GLES émulé par llvmpipe : Skia non sélectionnable sur Linux |
| egui 0.37 (Rust) | mesurée, **conservée avec réserve** | pas de support iOS réel (décision utilisateur : on le garde comme repère Rust pur) |
| NSH 0.2.0 (Rust seul, backend GTK 4) | mesurée | projet de 12 jours d'historique |
| iced 0.14 | **écarté** | mobile hors scope pour ses auteurs ; résultats gardés en annexe (`results/iced-*`), non classés |
| NSH en TSX/QuickJS, GPUI, Tauri, Dioxus WebView | **exclus** | décision utilisateur : pas de version TS/web ; ni mesurés ni classés |
| Swift/SwiftUI, Zig (DVUI, zigui), Dear ImGui, React Native, Lynx, MAUI, Uno | recherche seule | non implémentés (voir `docs/research/`) |

## 3. Recherche (architecture, maturité, licence, contributeurs)

Les fiches complètes, avec **URL et date de consultation de chaque source**, sont dans `docs/research/rust-zig-cpp.md` et `docs/research/dotnet-kmp-flutter-qt-swift.md`. Les contributeurs sont comptés par `git shortlog -sn --since=12.months` sur des clones sans blobs (bots et doublons d'identité non filtrés ; détails dans les fiches). Extraits (déclaré par les projets, niveau de preuve dans les fiches) :

| Stack | Licence | Auteurs 12 mois (commits) | iOS / Android / macOS / Linux / Windows / web | Notes |
|---|---|---|---|---|
| Slint 1.18.1 (2026-09-21) | GPL-3.0 **ou** Royalty-free-2.0 (attribution obligatoire, embarqué interdit) **ou** commerciale | 148 (5 173) | déclaré partout ; web = canvas WebGL, « non recommandé pour apps web générales » | pas d'API C stable documentée |
| Qt 6.12 LTS (2026-09-30) | LGPL-3 / GPL / commerciale | qtbase : 279 (113 avec ≥5 commits) | déclaré partout, wasm officiel | |
| Avalonia 12.1.4 (2026-10-09) | MIT | 179 (27 avec ≥5 commits) ; 895 commits | tiers 1 : Windows, macOS, iOS, Android, Linux ; mobile exige .NET 10 ; wasm annoncé | NativeAOT documenté (compiled bindings obligatoires) ; support AOT d'Avalonia.FuncUI non établi |
| Compose Multiplatform 1.12.2 (2026-10-15) | Apache-2.0 | dépôt CMP : 33 (le code UI est dans compose-multiplatform-core, non compté) | Android, iOS, desktop JVM : stables ; **Web (Wasm) : Beta** ; pas de desktop natif sans JVM officiel | `compose-native` communautaire : alpha, 1 auteur |
| Flutter 3.47.8 (2026-10-23) | BSD-3 | 411 (104 avec ≥5 commits ; un bot fait 1 972 commits) | déclaré partout ; web : Skia | Impeller par défaut sur macOS/Linux/Windows depuis 3.47 |
| egui 0.37.0 (2026-10-15) | MIT ou Apache-2.0 | 144 (563) | **pas d'iOS déclaré** ; Android via eframe ; web oui | accessibilité faible (README) |
| NSH 0.2.0 (2026-10-22) | Apache-2.0 | 20 (408) ; 71 % des commits par 2 personnes ; **premier commit 2026-10-14** | déclaré partout, vues natives (UIKit, vues Android, AppKit, GTK 4, Win32/D2D), web = DOM | aucune adoption externe vérifiée ; C-ABI documentée = embarquement iOS/Android seulement |

Chiffres de taille/RAM trouvés dans des sources (par ex. NSH : 49 Mo privés / 142 Mo RSS au repos, runner 16,5 Mo ; Slint : « < 300 Kio » de RAM ; Avalonia : démarrage Android 1 960→460 ms) sont des **mesures d'auteurs ou du marketing, non reproduits ici** — voir `docs/research/*`.

## 4. App de référence et protocole

Spécification : `docs/BENCH-SPEC.md`. Résumé : fenêtre 1280×720, release, rendu logiciel ; données déterministes (PRNG xorshift, hauteurs `48+16·(fnv(i)%6)`, somme attendue 8 799 584 vérifiée par chaque app) ; 9 scénarios (idle, list, stream, canvas, text, images, nav, core, stress).

- **Mesure externe** (`harness/run_bench.py`) : Xvfb neuf par exécution, CPU et RSS lus dans `/proc` à 10 Hz sur tout l'arbre de processus, une seule stack à la fois sur la machine, 2 s de warm-up.
- **Plan réellement exécuté** : phase A (idle, list, stream 10k, canvas 5 000, core) en **10 exécutions** ; phase B (stream 1k, canvas 20 000, text, images, nav) en **3 exécutions** ; list et canvas sur **1 cœur** en 3 exécutions ; **stress de 300 s en 1 exécution** (au lieu de 600 s × 10 prévu : réduction décidée pour gagner du temps ; les dérives mémoire sur 10 minutes et le throttling thermique ne sont donc **pas** observés). Démarrage : 10 exécutions à froid (caches vidés) et à chaud.
- **Compose** a d'abord été construit en 1.5.12 (Google Maven était bloqué), puis refait en 1.12.2 quand l'hôte a été rouvert ; les deux jeux de mesures existent.

### Vérification que ce sont de vraies GUI
22/24 vérifications réussies (`harness/verify_gui.py`, `results/verification.jsonl`, captures dans `results/verification/`) : serveur X neuf avec l'app comme seul client ; 0 fenêtre avant le lancement, une fenêtre ≥1000×600 pendant, 0 après l'arrêt ; framebuffer non vide et changeant. **Exceptions** : Avalonia (AOT et JIT) en `text` n'a aucune fenêtre avant ~10–20 s (layout du Mo de texte dans un `TextBox`), puis s'affiche. Non prouvé : que chaque capture corresponde pixel pour pixel à la spec.

### Définition d'une « frame » et calibration externe
Les définitions varient (début de callback chez Avalonia, fin du raster chez Flutter, signal de rendu chez Qt/Slint, `GdkFrameClock` plafonné à 60 Hz chez NSH). Calibration par compteur externe des changements du framebuffer pendant `list` (le compteur consomme un peu de CPU, d'où des FPS applicatifs plus bas qu'en mesure officielle) :

| Stack | FPS déclaré par l'app (`list`) | Mises à jour visibles du framebuffer (/s) | Rapport |
|---|---|---|---|
| slint | 556 | 687 | 1.2× |
| avalonia-aot | 166 | 494 | 3.0× |
| avalonia-jit | 187 | 466 | 2.5× |
| compose-jvm | 212 | 427 | 2.0× |
| qt-software | 830 | 899 | 1.1× |
| flutter | 57 | 69 | 1.2× |
| egui-glow | 199 | 223 | 1.1× |
| nsh-gtk | 60 | 86 | 1.4× |

Les FPS sont donc **des ordres de grandeur**, pas des unités comparables à l'image près. Pour NSH, le pourcentage de « frames > 16,7 ms » est un artefact du plafond 60 Hz.

## 5. Résultats


#### Stacks retenues (phase A : 10 exécutions, médianes ; stress : 1 exécution)

| Stack | RAM idle (Mo) | CPU idle (% cœur) | list 100k : FPS (frame p95 ms) | stream 10k/s : FPS | canvas 5000 : FPS | core : M triplets/s (FPS UI) | 1er pixel froid / chaud (ms) | Stress 300 s : FPS, RSS début→fin (Mo) |
|---|---|---|---|---|---|---|---|---|
| slint | 23 | 0.16 | 528 (2.5) | 495 | 18.4 | 78 (201) | 88 / 46 | 40.6, 36→36 |
| qt-software | 52 | 0.25 | 798 (2.0) | 738 | 13.6 | 127 (67) | 134 / 66 | 13.9, 63→65 |
| avalonia-aot | 40 | 0.32 | 202 (6.8) | 190 | 8.5 | 37 (39) | 118 / 74 | 7.0, 71→92 |
| avalonia-jit | 79 | 5.66 | 178 (7.7) | 172 | 8.0 | 67 (35) | 867 / 813 | non mesuré |
| egui-glow | 114 | 2.91 | 197 (6.6) | 216 | 11.4 | 167 (63) | 154 / 99 | 10.2, 176→178 |
| nsh-gtk | 116 | 0.63 | 60 (17.9) | 60 | 4.0 | 106 (55) | 179 / 100 | 7.9, 149→149 |
| compose-jvm | 168 | 10.75 | 225 (6.7) | 216 | 4.7 | 25 (30) | 1688 / 1312 | 3.1, 318→467 |
| flutter | 243 | 1.63 | 50 (26.1) | 54 | 0.3 | 51 (2) | 443 / 317 | 0.3, 1662→1718 |

Secondaires (phase A : 3 exécutions) et hors périmètre plateformes (iced : mobile hors scope pour ses auteurs, 10 exécutions) :

| Variante | list FPS | stream 10k FPS | canvas 5000 FPS | core FPS UI | RAM idle Mo |
|---|---|---|---|---|---|
| slint-femtovg | 173 | 162 | 1.4 | 8 | 135 |
| qt-opengl | 250 | 240 | 13.8 | 48 | 156 |
| egui-wgpu | 136 | 133 | 11.4 | 51 | 145 |
| iced-natural | 26 | 26 | 0.3 | 17 | 16 |
| iced-fulldamage | 238 | 231 | 4.4 | 31 | 16 |

#### Autres scénarios (phase B : 3 exécutions ; FPS médian)

| Stack | stream 1k/s | canvas 20 000 | text 1 Mo | images 600 | nav | list 1 cœur | canvas 5000 1 cœur |
|---|---|---|---|---|---|---|---|
| slint | 552 | 8.4 | 246 | 282 | 810 | 545 | 21 |
| qt-software | 774 | 3.5 | 172 | 398 | 710 | 866 | 14.3 |
| avalonia-aot | 201 | 2.5 | 1.2 | 110 | 170 | 201 | 8.8 |
| avalonia-jit | 192 | 2.2 | 1.1 | 110 | 160 | 178 | 8.1 |
| egui-glow | 206 | 4.2 | 279 | 176 | 132 | 83 | 4.2 |
| nsh-gtk | 60 | 1.1 | 60 | 60 | 56 | 60 | 4.0 |
| compose-jvm | 201 | 1.3 | 118 | 28 | 245 | 145 | 4.7 |
| flutter | 55 | **aucune frame** | 35 | 54 | 33 | 36 | 0.6 |


Lecture :
- **list / stream** : Qt 798/738 FPS et Slint 528/495 FPS, loin devant Compose 225/216, Avalonia AOT 202/190 et egui 197/216 ; NSH (plafonné 60 Hz) et Flutter (50/54) en dessous. 0 mise à jour perdue à 10 000/s partout.
- **canvas 5 000** : Slint 18,4 · Qt 13,6 · egui 11,4 · Avalonia 8,5 · Compose 4,7 · NSH 4,0 · Flutter 0,3. À 20 000 formes : Slint 8,4 · egui 4,2 · Qt 3,5 · Avalonia 2,5 · Compose 1,3 · NSH 1,1 · Flutter **aucune frame en 14 s**.
- **text 1 Mo** : egui 279, Slint 246, Qt 172, Compose 118, NSH 60 (plafond), Flutter 35, **Avalonia 1,2** (≈12 s avant le premier affichage, ≈1 Go de RAM).
- **images (600 PNG)** : Qt 398, Slint 282, egui 176, Avalonia 110, NSH 60, Flutter 54, **Compose 28** (régression par rapport à 107 FPS en 1.5.12, non expliquée).
- **1 cœur** : le défilement tient presque partout ; egui chute (197→83 FPS) car llvmpipe parallélise le GL.
- **stress 300 s** : Slint ≈41 FPS ; Qt ≈14 ; egui ≈10 ; NSH ≈8 ; Avalonia AOT ≈7 ; Compose ≈3 ; Flutter ≈0,3. RSS : stable pour Slint/Qt/egui/NSH, +21 Mo pour Avalonia AOT, **+149 Mo pour Compose** (318→467), ~1,7 Go pour Flutter.
- **Appels C-ABI** (`core`, mesuré pendant que l'UI anime 1 000 formes, donc *pas* un coût d'appel pur) : egui 167, Qt 127, NSH 106, Slint 78, Avalonia JIT 67, Flutter 51, Avalonia AOT 37, Compose 25 M triplets/s (validate+parse+hash). Les agents ont aussi relevé pour Avalonia 20–28 M/s (P/Invoke dynamique) jusqu'à 86–91 M/s (liaison statique + `SuppressGCTransition`), mesures bruitées (machine chargée). Référence C→Rust : ≈120 M/s.

### Empreinte disque (relevée par les agents, non re-mesurée séparément)
egui 7,2 Mo · Slint 16 Mo · Flutter 21,7 Mo (bundle) · NSH 22,5 Mo (+≈37 Mo de bibliothèques GTK système) · Avalonia AOT 34,2 Mo · Qt ≈90–100 Mo (estimation, libs Qt 28,6 Mo + ICU 36 Mo + binaire 5,2 Mo) · Avalonia JIT 106,9 Mo · Compose 134 Mo (runtime jlink 85 Mo). Tailles wasm (brotli) : **non mesurées**.

### Limites propres à chaque stack (résultats, pas échecs du test)
- **Slint** : le rendu logiciel ne dessine ni les ombres portées (`draw_box_shadow` est un `TODO` dans le moteur) ni les dégradés arrondis (sortis carrés) ; son canvas fait donc moins de travail. `ListView` ne gère que les hauteurs fixes → virtualisation écrite à la main. Winit/X11 limite à 60 Hz ; le throttle a été contourné pour la mesure.
- **Qt** : `QtQuick.Templates` absent → `nav` écrit en QtQuick pur ; en backend software les signaux de frame sont émis sans changement à l'écran (compensé dans l'app, documenté dans `notes`).
- **Avalonia** : plafonné à 60 Hz sur X11 sans hook public ; contourné par réflexion sur une API privée (fonctionne en AOT) pour mesurer le débit maximal ; non représentatif d'une app livrée. Coût du dégradé par forme ≈18 µs.
- **egui** : clip rectangulaire seulement (pas d'arrondi), dégradés par `Mesh` sans anticrénelage, emoji ZWJ en tofu, pas de « tout sélectionner » sur texte virtualisé.
- **Flutter** : Impeller/GLES sur llvmpipe (Skia indisponible : `FLUTTER_LINUX_RENDERER=software` plante avec du texte : `Impeller DlText cannot be drawn to a Skia canvas`, rc=134) ; cadencé ≈60 Hz par l'embedder GTK ; résultats de canvas sans valeur pour un vrai GPU.
- **NSH** : cadencé à 60 Hz par `GdkFrameClock` ; l'ombre naïve du canvas coûte ≈6 s/frame (contournée par un clip à la boîte de la forme, documenté) ; liste virtualisée sans API de hauteur exacte (étendue estimée).
- **Compose** : FFM de `java.lang.foreign` en preview sur le JDK 21 ; `select-all` du texte non supporté ; compilation avec 1.12.2 sans modification de code.

## 6. Portes F\*

| Porte | Étape | Résultat |
|---|---|---|
| F\* → F# → .NET 10 → NativeAOT | spec 177 lignes vérifiée (`fstar.exe` 2025.03.25, Z3 4.13.3) | OK |
| | extraction `--codegen FSharp` | OK mais le F# brut ne compile pas : 3 erreurs (littéraux `u8` mal typés car `FStar_UInt8.t` = `bigint` ; code de spec extrait dépendant de `FStar_UInt` absent ; `#light "off"` refusé par F# récent → `LangVersion 5.0`) ; 2 contournements côté source F\* (`noextract`, `U8.uint_to_t`) |
| | build .NET 10 + 9 201 vérifications de tests | OK (JIT et AOT) ; la suite détecte une mutation volontaire |
| | NativeAOT | OK : 3,4 Mo, 11 ms de démarrage à froid, 5,5 Mo de RSS idle ; warnings `IL3053`/`IL2104` (FSharp.Core) |
| | support F# officiel (`fsharp/` du dépôt F\*) | README : « not tested by anything in this repository » ; `FStar_UInt8.t` est un `bigint` ; hash de 64 octets ≈7–13 µs |
| F\* (Low\*) → C via KaRaMeL | Core_Low.fst 209 lignes vérifiées ; mutation 64→65 rejetée | OK |
| | construction de KaRaMeL | d'abord avec un shim `fix` écrit à la main (hôtes bloqués) ; **refaite sans aucun contournement** après réouverture d'opam/gitlab.inria.fr : paquets réels (`fix` 20250919, `wasm` 2.0.2, `process`, `stdint`), commit karamel 98ffca5, ≈3 min ; le C généré est **identique** à celui du shim (hors commentaire d'en-tête) |
| | C généré | 57 lignes, sans dépendance, compile en `-Wall -Wextra -Werror` ; 2 634 vérifications + ASan/UBSan, 0 divergence sur 2 000 000 d'entrées contre le cœur Rust |
| | contournements restants | règles d'écriture Low\* (`noextract`, `inline_for_extraction`, extraction de `C.Loops`), pas de défaut de build |
| | débit | non conclusif (mesuré sous charge) : ≈0,87× le Rust en appel direct, ≈0,59× via adaptateur ABI ; l'adaptateur refuse n > UINT32_MAX |

Détails, erreurs exactes et logs : `docs/fstar/RAPPORT-FSHARP.md`, `docs/fstar/RAPPORT-KRML.md`, `fstar/`, `stacks/karamel/`. Aucun numéro d'issue F\* n'a pu être consulté (API GitHub inaccessible) : aucun n'est cité comme vérifié.

## 7. Tableau comparatif (jugement synthétique)

Les colonnes « perf », « empreinte » viennent de nos mesures *logicielles Linux* ; « plateformes », « wasm », « maturité » viennent de la recherche (déclaré, non testé par nous) ; « DX » et « risque » sont des jugements fondés sur ce que les agents ont rencontré en construisant l'app.

| Stack | Perf sous charge (mesurée) | Empreinte (mesurée) | Plateformes | wasm | DX (observée) | Maturité | Risque principal |
|---|---|---|---|---|---|---|---|
| Slint | très bonne (stress 41 FPS) | très faible (23 Mo, 16 Mo disque) | 6 déclarées | oui mais déconseillé pour apps web générales | liste à hauteur variable et canvas à écrire à la main | bonne | licence (GPL / Royalty-free avec attribution / commerciale), rendu logiciel limité (ombres) |
| Qt | la meilleure en défilement | faible RAM, disque ≈90–100 Mo | 6 déclarées | officiel | deux langages (C++ + QML) | très élevée | licence LGPL/commerciale, taille |
| Avalonia AOT (F#) | bonne (list 202), canvas lourd, texte 1 Mo injouable | faible (40 Mo, 34 Mo disque) | 6 déclarées (mobile : .NET 10) | annoncé | un seul langage (F#) ; AOT + F# fragile | élevée (desktop) | 60 Hz non désactivable par API publique, AOT d'Avalonia.FuncUI non établi |
| egui | bonne, CPU élevé | moyenne | **pas d'iOS** | oui | immédiat, tout à écrire | moyenne | couverture mobile, accessibilité |
| NSH (Rust) | moyenne, plafonné 60 Hz, CPU bas | moyenne | 6 déclarées | oui (DOM) | vues natives, API jeune | **très faible (12 jours)** | maturité, bus factor |
| Compose (JVM) | correcte en défilement, faible en canvas/stress | lourde (168 Mo, 134 Mo disque, démarrage 1,7 s) | Android/iOS/desktop stables | Beta | un seul langage (Kotlin) | élevée | JVM, dérive mémoire, Web en Beta |
| Flutter | faible en rendu logiciel (artefact probable) | lourde (243 Mo idle) | 6 déclarées | oui | un seul langage (Dart) | très élevée | non évaluable sans GPU |

## 8. Classement et recommandations

**Classement pour vos priorités (perf → plateformes → un langage → cœur vérifié → DX), sur les preuves actuelles :**
1. **Slint** — meilleur rapport perf/empreinte mesuré, Rust, C-ABI triviale. Conditions de chute : licence incompatible, besoin d'ombres/effets en rendu logiciel, web comme cible principale, maturité mobile non vérifiée.
2. **Qt/QML** — meilleure perf brute et maturité maximale. Conditions de chute : refus de deux langages, contraintes LGPL, taille.
3. **Avalonia en NativeAOT (F#)** — bon compromis, un seul langage, bon démarrage. Chute si : textes très longs, canvas lourd, ou AOT + F# bloquant.
4. **Compose Multiplatform** — rattrape la liste, mais RAM/démarrage/dérive mémoire. Remonte si : l'équipe est Kotlin d'abord et mobile d'abord.
5. **Flutter** — non classable honnêtement ici : nos chiffres reflètent un GLES émulé. À remesurer avec GPU.
6. **NSH** — trop jeune pour un choix de production ; à surveiller.
*egui* est un repère Rust pur (hors couverture iOS), *iced* est écarté.

**Ce qui ferait changer le classement** : un GPU réel (remonte Flutter, Compose, Slint-Skia/FemtoVG, egui-wgpu), une cible web prioritaire (Compose/Flutter/Qt), une contrainte de licence (exclut Slint/Qt selon l'usage), une équipe mono-langage imposée.

**Par type d'app (proposition, non validée sur appareils)**
- *Outil léger* (desktop, petite taille, démarrage rapide) : Slint, ou Avalonia AOT si l'équipe est .NET.
- *App riche* (listes énormes, texte, graphiques) : Qt, ou Slint ; Avalonia si le texte long est limité.
- *Mobile d'abord* : non tranché par nos mesures (aucun appareil) ; candidats Compose (iOS/Android stables) et Flutter, à mesurer sur appareil avec le protocole §10.

## 9. Plan de migration depuis KMP (proposition, non testée sur mobile)
1. **Inventaire** : séparer dans `commonMain` ce qui est logique métier pure (validation, parsing, hash, règles) de ce qui est UI/plateforme.
2. **Contrat C-ABI** : définir l'interface (`validate`, `parse`, `hash`, comme `core/benchcore.h`) ; ordre de grandeur mesuré en JVM via FFM : ≈25 M triplets/s sous charge UI.
3. **Cœur en Rust (ou F\* Low\* → C)** : réimplémenter/extraire le cœur ; le prouver avec F\* si la sécurité l'exige (porte Low\*→C validée ici).
4. **Branchement dans KMP d'abord (strangler)** : appeler le cœur depuis Kotlin (FFM/JNI sur JVM et Android, cinterop sur iOS) sans changer l'UI ; non testé iOS/Android ici.
5. **Tranche verticale** dans la stack cible (par ex. Slint ou Qt) avec le même cœur, puis migration écran par écran derrière la navigation.
6. **Bascule** quand la parité fonctionnelle et les budgets perf (protocole §10) sont atteints ; garder l'UI KMP en parallèle jusque-là.

## 10. Non mesurable dans le cloud, et protocole à exécuter chez vous
Non mesuré ici : GPU, iOS/Android réels, IME (CJK, emoji, bidi), énergie, thermique, latence d'entrée de bout en bout, wasm (taille brotli, démarrage), Windows et macOS, dérive sur 10 minutes.

Protocole : (1) réimplémenter ou réutiliser `stacks/<stack>` avec la même spec (`docs/BENCH-SPEC.md`) ; (2) iOS : Instruments (Time Profiler, Core Animation FPS, Allocations) sur un iPhone d'entrée de gamme, 10 exécutions à froid ; (3) Android : `adb shell dumpsys gfxinfo <pkg> framestats`, Perfetto, `dumpsys meminfo`, `bundletool` pour la taille installée ; (4) Windows : WPR/WPA et PresentMon ; macOS : Instruments ; (5) web : DevTools Performance, tailles brotli du `.wasm` et du JS ; (6) IME : saisie réelle en japonais/chinois avec composition, emoji ZWJ, texte arabe/hébreu (curseur, sélection) ; latence : caméra 240 fps ou capteur photodiode ; (7) stress de 10 minutes avec échantillonnage mémoire toutes les 10 s ; (8) mêmes statistiques : médiane et p95 sur 10 exécutions.

## 11. Annexes
- Arborescence : voir `README.md`.
- Sources web avec URL et date : `docs/research/*.md` (sections « Sources »).
- Journaux bruts : `results/*.json`, `results/*.stdout`, `results/driver-part*.log`, `logs/`, `stacks/*/logs/`.
- Reproduction : `harness/run_bench.py`, `harness/run_all.sh`, `harness/run_resume.sh`, `harness/run_trim2.sh` (plan réduit réellement exécuté), `harness/coldstart.py`, `harness/calibrate.py`, `harness/verify_gui.py`, `harness/aggregate.py`, `harness/synthese.py`.
