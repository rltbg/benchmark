# Porte 2 : F* (Low*) -> KaRaMeL -> C -> FFI : rapport mesuré

Date : 2026-10-25. Tout ce qui est marqué OK a été exécuté ; logs bruts dans `logs/`. Pipeline reproductible : `./build.sh` (env dans `env.sh`).

## Verdict net (mis à jour après reconstruction propre)
**Utilisable : la construction propre de krml est maintenant reproductible sans aucun contournement de build, et le C généré est inchangé.**
Les hôtes `opam.ocaml.org` et `gitlab.inria.fr` répondent désormais ; opam installe les vrais `fix` 20250919, `wasm` 2.0.2, `process` 0.2.1, `stdint` 0.7.2 et le clone git de karamel se construit avec `make local-install` (krmllib vérifiée par F* comprise) en 1 min 3 s après 2 min 7 s d'installation opam des dépendances. Le shim `fix`, `vendor/process`, `vendor/wasm`, le `Version.ml` manuel et le `sysroot` de .deb ne servent plus (shims supprimés ; la section « Bugs » plus bas décrit le premier essai, historique).
Contreparties restantes : (1) il faut une version de F* compatible avec le karamel de la tête de branche : ici F* 2025.03.25 + karamel 98ffca5 fonctionnent ensemble, sans garantie générale ; (2) la spec/preuve est un vrai travail (209 lignes F* pour 57 lignes de C) ; (3) le débit mesuré avant reconstruction est inférieur au Rust (voir mesures, bruitées) ; (4) les contournements #5 à #10 (côté source F* et options gcc) restent nécessaires : ce sont des règles d'écriture Low*, pas des défauts de build.

## Mise à jour : reconstruction propre (sans contournements #1 à #4)
Hôtes testés depuis ce conteneur (`logs/80-hosts-recheck.log`) : 200 : `opam.ocaml.org/index.tar.gz`, `gitlab.inria.fr/fpottier/fix` et son `-/archive/...tar.gz`, `ocaml.org`, `deb.debian.org/debian/`, `raw.githubusercontent.com` ; 404 : `opam.ocaml.org/cache/` (répertoire, sans effet) ; 403 : `github.com/ocaml/opam-repository`, `codeload.github.com/...`, `github.com/WebAssembly/spec/archive/opam-2.0.2.zip`, `crates.io`. Le paquet `wasm` s'est tout de même installé (opam a trouvé l'archive par un autre chemin, probablement le cache opam ; je n'ai pas isolé lequel).
Commandes exactes (OPAMROOT local `opamroot/`, sans apt, sans sandbox opam) :
1. `opam init --bare --disable-sandboxing --no-setup` puis `opam switch create krml ocaml-system` : 27 s (`logs/81`).
2. `opam install batteries zarith stdint yojson fileutils pprint process fix visitors "wasm>=2.0.0" ppx_deriving ppx_deriving_yojson ctypes ctypes-foreign uucp sedlex menhir ocamlbuild ocamlfind dune ppxlib` (OPAMJOBS=2) : 2 min 7 s, 0 erreur (`logs/82`). Le paquet opam `fstar` n'a pas été installé (le F* de la release binaire est utilisé).
3. `git clone --depth 1 https://github.com/FStarLang/karamel karamel-clean` (commit 98ffca5, identique), `make minimal -j2` : 12 s, `lib/Version.ml` généré par le Makefile (`logs/83`).
4. `make local-install -j2` (avec le `fstar.exe` de la release dans le PATH) : 1 min 3 s, krmllib + `out/` (`logs/84`).
5. `./build-clean.sh` (même pipeline que `build.sh` avec `karamel-clean/out`) : 3,3 s car cache F* chaud. `krml-clean/out.krml` est **octet à octet identique** à celui de la version shim (`cmp`), `gen-clean/Core_Low.c/.h` ne diffèrent que par la ligne de commande dans l'en-tête de commentaire (`diff` : chemin du `krml` et du répertoire), `Core_Low.o` identique (`cmp`). Donc le shim `fix` n'avait pas changé la sortie sur ce module (preuve sur ce seul module, pas en général).
Tests relancés avec le krml propre : 2634 vérifications, 0 échec, 0 divergence vs Rust sur 2 000 000 entrées, idem sous ASan+UBSan (`logs/99-build-clean.log`).
Mesure de débit : **non refaite**. La machine n'est pas calme (loadavg ~3 avec un build `compose` à 160 % CPU en cours) ; les chiffres du tableau « Mesures » ci-dessous datent d'avant et sont bruités : aucune conclusion de ratio n'en est tirée. Le binaire testé étant identique octet à octet, une re-mesure ne changerait que le bruit.
Premier essai (historique) : tableaux ci-dessous.

## Environnement
F* 2025.03.25 (release binaire, Z3 4.13.3 intégré), `ulib` contient bien `LowStar.*` (28 modules) et `FStar.HyperStack.ST`. **`Lib.IntTypes` (HACL*) absent** : utilisé `FStar.UInt8/UInt32` + `LowStar.Buffer`.
KaRaMeL : `git clone --depth 1` anonyme, commit `98ffca53b25c8ac50f303dae87c083de65cc1db8` (`logs/10`). OCaml 4.14.1, dune 3.14, Ubuntu 24.04. `C.Loops` vient de `krmllib/` du dépôt KaRaMeL (absent de la release F*). 4 vCPU, charge moyenne 5 à 7 pendant les mesures (autres agents).

## Tableau par étape
| # | Étape | Résultat | Preuve |
|---|---|---|---|
| 1 | Module Low* `Core_Low.fst` (209 lignes, 182 non vides) | **OK** : « All verification conditions discharged successfully » (~7 s) | `30-fstar-verify-6.log`, `50-fstar.log` |
| 2a | KaRaMeL via apt (1er essai) | **KO partiel** : apt fournit ppxlib, ppx_deriving(_yojson), yojson, pprint, menhirLib, sedlex, uucp, visitors ; **pas** `fix`, `wasm`, `process` (seulement en resolute/devel, OCaml incompatible), `stdint` | `11`, `12`, `15` |
| 2b | opam + dépôt local | **Non tenté jusqu'au bout** : `raw.githubusercontent.com` répond (index opam lisible) mais les archives sources sont sur `github.com/.../archive` (403), `gitlab.inria.fr` (CONNECT 403), `opam.ocaml.org` (000) | `13-hosts.log` |
| 2c | Dépendances à la main (1er essai, remplacé par opam, voir mise à jour) | **OK avec contournements #1 à #4** : `krml` (Karamel.exe 33 Mo) construit par dune, 0 erreur | `20`, `21-build-karamel-3.log`, `42` |
| 3 | `fstar.exe --codegen krml` -> `krml/out.krml` (3,9 Ko) | **OK après 4 contournements côté source** (#5 à #8) | `40-krml-1..5` |
| 4 | `krml` -> `gen/Core_Low.c/.h` | **OK** (57 + 23 lignes) | `51-krml.log`, `gen/` |
| 5 | gcc -O2 `-Wall -Wextra -Werror` -> `.o/.a/.so` | **OK** (avec `-D_DEFAULT_SOURCE`, #9) | `52-cc.log`, `lib/` |
| 5b | Compilation intégrée de krml (`krml ... -o x.a`) | **Partiel** : .o OK avec `-Wall -Werror`, l'édition de liens exige `libkrmllib.a` (non construit) ; `-skip-linking` suffirait. Non utilisé | `41-krml-fullcompile.log` |
| 6 | Tests C (2634 vérifications) + différentiel vs Rust (2 000 000 entrées) | **OK : 0 échec, 0 divergence** ; idem sous ASan+UBSan | `53`, `54` |
| 7 | Mutations (la suite et les preuves détectent-elles ?) | **OK** : `64uy`->`65uy` rejeté par F* (Error 19) ; constante de hash fausse rejetée par F* (Error 19 sur le lemme contre la spec entière) ; `64U`->`65U` dans le C : 2 échecs + 9614 divergences | `55`, `56` |
| 8 | Micro-banc triplets/s | **OK** (bruité) | `70`, `70b`, `71*` |

## Ce qui est prouvé (F*, vérifié)
Spec sur `Seq` : `well_formed`, `parse_spec`, `encode_spec`, `hash_spec` (fold FNV-1a), `hash_nat` (spec en entiers non bornés, indépendante).
- `validate` : `Stack`, `h0 == h1` (ne modifie pas la mémoire), `r <==> well_formed (view h0 b len)`. Précondition `live b /\ len <= length b` ; chaque `B.index` est vérifié dans les bornes par le typeur ; `3 + n` en U32 sans débordement (précondition de `U32.add` prouvée, n <= 64 < 256).
- `parse` : `modifies` limité à `out` et `out_len` ; `r <==> (parse_spec s = Some p /\ |p| <= cap)` ; si `r` : `out_len = |p| <= 64`, `out[0..n) = p` ; `blit` exige disjonction et bornes (prouvées).
- `hash` : `modifies loc_none`, `r == hash_spec s` et `v r == hash_nat s` (aucune perte par `mul_mod`) ; invariant de boucle via `C.Loops.for`.
- Lemmes purs : `parse_length_bound` (payload <= 64, trame <= 67), `parse_rejects_short`, `roundtrip`, `hash_correct`, `hash_snoc`, vecteurs `""` (0x811C9DC5), `"a"` (0xE40C292C), `"foobar"` (0xBF9CF968).
Ce qui n'est **pas** prouvé : la terminaison/bornes de l'adaptateur C `kr_shim.c` (24 lignes à la main) ; l'équivalence `size_t` -> `uint32_t` (n > 2^32-1 refusé pour validate/parse, hash renvoie l'offset : divergence connue avec le Rust sur de telles entrées).

## Bugs et contournements (numérotés, erreurs exactes dans les logs)
1. **Dépendance `wasm`** : le dépôt `WebAssembly/spec` (clone anonyme OK) est en 3.0 sur master ; `error: Unbound value W.Ast.string_of_name` (`21-build-karamel-1.log`). Contournement : tag `opam-2.0.2` (équivalent du paquet `wasm` 2.0.x), copié en `vendor/wasm` avec un `dune` réduit (`vendor-shims/`).
2. **Dépendance `fix` introuvable** (gitlab.inria.fr bloqué, pas dans Ubuntu). Contournement : **shim écrit par moi** (`vendor-shims/fix/fix.ml`, ~40 lignes : itération de Kleene naïve à la demande, API `Fix.Fix.ForOrderedType`, `Fix.Make`, `Fix.Prop.Set`). Risque : sémantique d'ordre d'évaluation différente de la vraie Fix ; utilisée par Inlining, DataTypes, et les backends Rust. Premier jet avec mauvais ordre d'arguments (`Error: This expression has type lident -> visibility but ...`), corrigé. **Le C généré n'a pas été comparé à celui d'un vrai `fix`.**
3. **Dépendance `process`** : clone anonyme de `dsheets/ocaml-process` OK, 2 fichiers copiés en `vendor/process`.
4. **`lib/Version.ml` absent** : généré par le Makefile (`Error: Unbound module Version`) ; écrit à la main (`logs/Version.ml.txt`). `stdint` n'est pas référencé par les `dune` : pas nécessaire. Paquets Ubuntu utilisés **sans apt install** : 22 `.deb` téléchargés par curl depuis archive.ubuntu.com et extraits (`dpkg-deb -x`) dans `sysroot/` (`logs/15`), exposés via `OCAMLPATH`.
5. **Fonctions de spec extraites** : `Warning 2: Reference to FStar.Seq.Base.length has no corresponding implementation` (fatal). Contournement : `noextract` sur `well_formed`, `parse_spec`, `encode_spec`, `hash_spec`, `hash_nat`, `fnv_step_nat`, `sn`.
6. **`FStar.UInt32.uint_to_t` non implémenté en C** (`Warning 2 ... FStar.UInt32.uint_to_t`) : remplacé par `FStar.Int.Cast.uint8_to_uint32`.
7. **`C.Loops.for` sans implémentation** (`Warning 2: Reference to C.Loops.for has no corresponding implementation`) : il faut extraire `C.Loops` avec le module (`--extract 'Core_Low C.Loops'`). `--extract_module` seul ne suffit pas ; la sortie est un unique `out.krml`.
8. **Constantes `nat`** : `Warning 15: Core_Low.header_len is not Low*` et `Warning 1/5 ... FStar.Seq.Base.seq, which is not defined` : `noextract` sur `max_payload`, `header_len`, `payload`. Sans `inline_for_extraction`, `magic/version/fnv_*` sortaient en variables globales mutables non inlinées ; corrigé par `inline_for_extraction noextract` (+ `[@@ CInline]` sur `fnv_step`).
9. **`-std=c11` strict** casse les en-têtes KaRaMeL (`implicit declaration of function 'htobe64'`, `-Werror`) : ajouter `-D_DEFAULT_SOURCE` (krml le fait lui-même dans sa ligne de commande).
10. **Preuves** : `assert_norm` sur `S.create 1 97uy` échoue (« Assertion failed ») car `Seq` est abstrait : remplacé par `hash_snoc` + `assert_norm` sur `fnv_step`. `h0 == h1` impossible pour `hash` (push/pop_frame) : post-condition `modifies loc_none`, qui suffit.
11. Divers non bloquants : `Warning 247 ... checked file ... C.Loops.fst.checked does not exist` (les `.checked` de krmllib ne sont pas produits, re-vérification à chaque appel) ; l'en-tête généré indique `F* version: <unknown>` ; `sysroot/` = 180 Mo de .debs extraits.
Aucun numéro d'issue GitHub n'a été consulté (web bloqué) : aucun n'est cité.

## Taille et lignes
| Élément | Valeur |
|---|---|
| F* `Core_Low.fst` | 209 lignes (182 non vides) ; spec Core.fst liste : 177 lignes |
| C généré `Core_Low.c` + `.h` | 57 + 23 = 80 lignes ; adaptateur `kr_shim.c` à la main : 24 lignes |
| `krml/out.krml` | 3 940 octets |
| `Core_Low.o` (gcc -O2) | 1 840 o (text 510 o) ; `libcorelow.a` 2 054 o ; `libcorelow.so` 15 616 o ; `libcorelow_shim.a` 4 090 o |
| Référence Rust `libbenchcore.a` | non comparée en taille (contient std) |
| Symboles exportés | `Core_Low_fnv_step`, `_hash`, `_parse`, `_validate` (`logs/73-nm.log`) ; aucun appel externe hors `memcpy` |

Ratio : 209 lignes F* (preuves comprises) pour 57 lignes de C, soit environ 3,7 pour 1.

## Mesures (gcc -O2, même boucle que `core/callbench.c`, 100 M triplets validate+hash+parse ; trame de 8 octets)
Mesures du premier essai, à considérer comme bruitées sans conclusion de ratio. Machine chargée (loadavg 5 à 7 sur 4 cœurs) : les valeurs absolues sont plus basses que la référence annoncée (~120 M/s) et bruitées. Passes entrelacées. Valeurs triées, M triplets/s.

| Variante | Série 1 (5 passes, sans taskset) | Série 2 (5 passes, `taskset -c 2`) |
|---|---|---|
| Rust à la main, lien statique (re-mesuré) | 77,9 / 88,5 / 93,6 / 96,2 / 105,4 | 80,4 / 85,7 / 94,1 / 100,5 / 123,5 |
| Low* -> C, `libcorelow_shim.a` (ABI `benchcore.h`, appel via shim) | 52,7 / 52,9 / 54,4 / 56,6 / 57,9 | 46,7 / 52,9 / 55,7 / 58,0 / 59,2 |
| Low* -> C, shim + code généré dans la même unité (inlinable) | 57,8 / 63,5 / 68,3 / 68,4 / 73,5 | 58,8 / 59,5 / 64,4 / 65,1 / 66,9 |
| Low* -> C, appel direct `Core_Low_*` (`uint32` len, hors ABI `benchcore.h`) | 73,8 / 75,3 / 79,8 / 87,1 / 94,9 | 71,9 / 80,5 / 81,8 / 85,5 / 92,1 |

Médianes série 2 : Rust 94,1 ; shim .a 55,7 (environ 0,59x) ; même unité 64,4 ; direct 81,8 (environ 0,87x). Les recouvrements d'intervalles Rust/direct montrent que la mesure ne permet pas de conclure finement sur l'écart direct ; l'écart du shim (un appel + tests + clamp de `cap` en plus) est net. Les `acc` des quatre binaires sont identiques (même résultat calculé).

## Alternative réaliste si krml n'est pas reproductible
Garder la spec F* comme source de vérité et extraire OCaml (`--codegen OCaml`, F* fournit son runtime) ou conserver le C généré `gen/Core_Low.c` comme artefact figé versionné (57 lignes, relisible, sans dépendance) en n'exécutant krml que sur une machine dédiée : le build de production n'a alors besoin que de gcc.

## Fichiers
`Core_Low.fst`, `gen/Core_Low.{c,h}`, `lib/*.a *.so`, `tests/`, `bin/`, `build.sh`, `env.sh`, `karamel-clean/` + `opamroot/` (construction propre), `build-clean.sh`, `env-clean.sh`, `gen-clean/`, `lib-clean/`, `krml-clean/` ; premier essai : `krml-prefix/`, `sysroot/` (shims supprimés), `logs/`.
