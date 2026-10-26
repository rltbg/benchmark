# Spécification du « spike » multiplateforme (identique pour Slint, Qt et Avalonia)

But : vérifier qu'**une seule base de code et un seul langage** produisent une vraie app sur les six plateformes — **Linux, Windows, macOS, iOS, Android, web (wasm)** — et mesurer ce que ça coûte (taille, temps de build, avertissements, effort de configuration). Ce n'est pas le banc de performance : c'est la preuve de portée.

## L'app (même contenu partout ; code UI partagé, seules les « têtes » de plateforme diffèrent)
Trois onglets (barre du bas sur mobile, barre du haut sur bureau/web) :
1. **Liste** : 100 000 lignes virtualisées à hauteur variable (`h(i) = 48 + 16*(fnv(i) % 6)` px, `fnv` = FNV-1a 32 bits des 4 octets little-endian de i), titre `Item #i`, prix à droite mis à jour 1 000 fois/s par une tâche de fond (xorshift32, graine 0x2545F491 : index = x % 100000, prix = x / 100.0). Défilement tactile/molette.
2. **Canvas** : 2 000 formes animées (1/3 rectangles, 1/3 cercles, 1/3 rect. arrondis, dégradé linéaire 2 couleurs, rebond), compteur de FPS superposé (frames sur la dernière seconde).
3. **Cœur** : un champ de saisie (clavier virtuel et IME sur mobile) ; le texte saisi est mis en trame `[0xA5][0x01][n][octets UTF-8, n ≤ 64, tronqué]` ; l'écran affiche `validate`, le `payload` extrait par `parse` et le `hash` (FNV-1a 32 bits, hexadécimal) ; un bouton « charge 100 ms » exécute `core_heavy`-équivalent (boucle de calcul de 100 ms) sur un fil de fond **pendant que le canvas de l'onglet 2 continue d'animer** (aucune saccade attendue).

Bandeau permanent : `stack · plateforme · cœur=<implémentation> · autotest=OK|ÉCHEC`.

## Le cœur vérifié (chaque stack le porte à sa manière, c'est une partie de la comparaison)
- Spécification : mêmes règles de trame que `core/src/lib.rs` (magic 0xA5, version 1, n ≤ 64, longueur exacte) et FNV-1a 32 bits.
- **Slint** : cœur Rust (crate `core/` en dépendance `path`, `rlib`) ; variante vérifiée = C généré par F\* Pulse (`stacks/karamel/gen/Custard.c`) compilé par la crate `cc`.
- **Qt** : C généré par F\* Pulse (`stacks/karamel/gen/Custard.c`, 66 lignes, aucune dépendance) compilé par CMake pour chaque cible ; aucune bibliothèque Rust à cross-compiler.
- **Avalonia (F#)** : F# extrait par F\* (`fstar/extracted/Core.fs` + `FStarCustard.fs`, inclus tels quels), pur managé donc identique sur toutes les cibles .NET.
- **Autotest au démarrage** : vecteurs FNV (`""` → 0x811C9DC5, `"a"` → 0xE40C292C, `"foobar"` → 0xBF9CF968) et trames valides/invalides (0, 64, 65 octets, mauvais magic, longueur incohérente) ; le résultat s'affiche dans le bandeau.

## Marqueurs de journal (pour que la CI vérifie que l'app tourne vraiment)
L'app écrit sur la sortie de journal de la plateforme (stdout/stderr, `logcat`, `os_log`/console du simulateur, `console.log` du navigateur), une ligne par événement, préfixée `SPIKE:` :
- `SPIKE:ready platform=<p> stack=<s> core=<impl>` à la première frame ;
- `SPIKE:selftest ok` ou `SPIKE:selftest fail <détail>` ;
- `SPIKE:fps tab=canvas value=<n>` chaque seconde quand l'onglet 2 est actif ;
- `SPIKE:done` à la fin du parcours automatique.
**Parcours automatique** (`autorun`) : onglet 1 pendant 3 s (défilement simulé), onglet 2 pendant 3 s, onglet 3 pendant 2 s (saisie simulée de `hello`, puis charge 100 ms), puis `SPIKE:done`. Activation : variable d'environnement `SPIKE_AUTORUN=1` (bureau), extra d'intent `autorun=1` (Android), argument de lancement `--autorun` (iOS, via `simctl launch`), paramètre d'URL `?autorun=1` (web). Sans autorun, l'app reste interactive.

## Plateformes et ce qui est attendu de la CI
| Plateforme | Runner GitHub | Livrable | Étape 2 (exécution) |
|---|---|---|---|
| Linux x64 | `ubuntu-24.04` | binaire ou archive | lancer sous Xvfb, capture, marqueurs |
| Windows x64 | `windows-2025` | `.exe` / dossier | lancer, capture d'écran, marqueurs |
| macOS arm64 | `macos-26` | `.app` | lancer, capture, marqueurs |
| iOS (simulateur arm64) | `macos-26` (Xcode 26.6, simulateurs iOS 26.x) | `.app` pour simulateur | `simctl install/launch`, capture, marqueurs |
| Android | `ubuntu-24.04` (SDK, NDK 27/29/30, KVM) | `.apk` (et `.aab` si simple) | émulateur (KVM) : installer, lancer, `logcat`, capture |
| Web (wasm) | `ubuntu-24.04` (Chrome 155 + chromedriver) | dossier statique (`.wasm`, `.js`, `.html`) | Chrome headless (Playwright ou chromedriver) : charger, console, capture |

Hors périmètre de la CI (à signaler, pas à simuler) : builds **signés** pour les magasins, iOS sur appareil réel, notarisation macOS, Windows ARM, Linux ARM.

## Résultats attendus par exécution
Pour chaque plateforme : `result.json` (via `ci/record.py` : statut, tailles brute/gzip/brotli, durée de build, notes), `build.log` complet (erreurs exactes), `run.log` (marqueurs `SPIKE:`), captures PNG, versions exactes des outils. Tout est copié dans un artefact `res-<stack>-<plateforme>` et publié dans la branche `ci-results` sous `<stack>/<sha>/<artefact>/` par l'action `.github/actions/publish-results`.
