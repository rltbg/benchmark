# Porte 2 : F* → C via KaRaMeL — passe 2 (dernières versions), rapport mesuré

Date : 2026-10-26. Tout ce qui est marqué OK a été exécuté. Logs bruts : `stacks/karamel/logs/v2/` (passe 1 archivée dans `logs/passe1/`, sources passe 1 dans `stacks/karamel/passe1/`).
Reproduction : `stacks/karamel/build.sh` (sorties dans `$BUILD=/opt/krml/p2/build` ; opam, karamel, LowStar et clone F* dans `/opt/krml`, hors dépôt).

## Verdict net
**Low\* → C est KO avec F\* v2026.10.25** : Low\* (`LowStar.*`, `FStar.HyperStack.*`) a été sorti de F\* le 2026-02-23 (commit `9d4e5c05`, vers `FStarLang/LowStar`), le modèle de tas supprimé le 2026-03-31 (`36941670`), et ni la krmllib de KaRaMeL HEAD ni le dépôt LowStar ne se vérifient avec cette version (erreurs exactes ci-dessous). `Core_Low.fst` ne peut donc pas être revérifié tel quel.
**Le chemin qui remplace Low\* fonctionne** : le même noyau réécrit en **Pulse** (livré avec F\*), extrait par Custard `--codegen KrmlC`, puis **krml HEAD** → C : vérifié, 66 lignes de C, `-Wall -Wextra -Werror` propre, **2634 vérifications / 0 échec, 0 divergence sur 2 000 000 entrées vs Rust**, idem sous ASan+UBSan, mutations détectées. Bonus : `len` est maintenant `size_t` de bout en bout (la divergence connue n > 2³²−1 de la passe 1 disparaît).

## Versions exactes (vérifiées le 2026-10-26)
| Élément | Version | Vérification |
|---|---|---|
| F* | 2026.10.25, commit `362e792b` | `git ls-remote --tags` : dernier tag `v2026.10.25` |
| Z3 | 5.3.0 (défaut de cette version, livré) | voir `fstar/logs/v2/01-versions.log` ; builds passés avec `--z3version 5.3.0` |
| KaRaMeL | HEAD `98ffca53b25c8ac50f303dae87c083de65cc1db8` (2026-10-20, « Merge PR #795 ») | `git ls-remote` ; **identique** au sous-module `karamel` épinglé par le tag F* et au `bin/krml` livré dans la release (`krml -version`). Même commit qu'en passe 1 |
| OCaml / opam | OCaml **4.14.1** système (switch opam `ocaml-system`), opam 2.1.5 | `karamel.opam` : `"ocaml" {>= "4.10.0"}` → pas besoin d'OCaml plus récent |
| Dépendances opam | batteries 3.11.0, zarith 1.14, stdint 0.7.2, yojson 3.0.0, fileutils 0.6.6, pprint 20230830, process 0.2.1, fix 20250919, visitors 20210608, wasm 2.0.2, ppx_deriving 6.2.0, ppx_deriving_yojson 3.10.0, ctypes(-foreign) 0.24.0, uucp 18.0.0, sedlex 3.8.1, menhir 20260209, ppxlib 0.39.0, dune 3.24.2 | `opam install` depuis opam.ocaml.org, 3 min 51 s, 0 erreur (`logs/v2/21`) |
| LowStar (essai) | `FStarLang/LowStar` `b45e92ee` (2026-02-24, dernier commit) | clone anonyme |
| gcc / Rust | gcc 13.3.0 ; cœur Rust reconstruit avec Rust/cargo **1.99.1** (`CARGO_TARGET_DIR=/opt/krml/rust-target`, `--locked`) | `logs/v2/05` |

## Tableau par étape
| # | Étape | Résultat | Preuve |
|---|---|---|---|
| 1 | `Core_Low.fst` (passe 1) + ulib F* 2026 | **KO** : `Error 134 … Namespace 'FStar.HyperStack.ST' cannot be found` | `30` |
| 1b | krmllib de KaRaMeL HEAD (`C.Loops.fst`) | **KO** : même `Error 134` (la krmllib dépend de `LowStar.Buffer`/`FStar.HyperStack.ST`) | `31` |
| 1c | + `--include` du dépôt `FStarLang/LowStar` | **KO** : `Error 168 at FStar.HyperStack.ST.fsti(43,14-43,14): Syntax error` (`new_effect GST = STATE_h mem`, syntaxe d'effet que F* 2026 n'accepte plus) | `32` |
| 2 | opam init + switch `ocaml-system` | OK, 30 s | `20` |
| 2b | `make -j2 minimal` (krml HEAD) | **OK**, 31,6 s | `22` |
| 2c | `make local-install` par défaut (krmllib) | **KO** : `error: unrecognized option '--cmi'` (option retirée de F* ; le Makefile de krmllib l'utilise) | `23` |
| 2d | `make LOWSTAR=false local-install` (option prévue par le Makefile : « fstar2 (with Pulse) is not expected to be able to build krmllib ») | **OK** (binaire + en-têtes, sans krmllib vérifiée) | `24` |
| 3 | Port Pulse `Core_Pulse.fst` (228 l.) : vérification | **OK** en 5,7 s, « All verification conditions discharged » | `50` |
| 4 | `--codegen KrmlC --custard_entry …validate/parse/hash` → `.krml` (4 804 o) | **OK** (exige un `.checked` préalable, sinon `Error 317`) | `40`, `50` |
| 5 | krml HEAD → `gen/Custard.c/.h` | **OK**, 0 warning ; sortie identique avec le `krml` livré dans la release F* (hors ligne de commande en en-tête) | `41`, `47` |
| 6 | gcc -O2 `-Wall -Wextra -Werror -std=c11 -D_DEFAULT_SOURCE` | **OK** ; sans `-D_DEFAULT_SOURCE` toujours KO (`implicit declaration of function 'le64toh'` dans les en-têtes krml) | `50`, `57` |
| 7 | Tests C (2634) + différentiel 2 M vs Rust | **OK : 2634 vérifications, 0 échec, 0 divergence** | `50` |
| 7b | Idem ASan+UBSan (`-fno-sanitize-recover=all`) | **OK : 2634 / 0 / 0**, aucun rapport | `50` |
| 7c | Idem sur la sortie `--codegen C` (C direct de Custard, sans krml ni krmllib) | **OK : 2634 / 0 / 0** (normal et ASan/UBSan) | `55` |
| 8 | Mutations F* : borne 64→65, constante FNV fausse, copie décalée 3→2, `out_len := cap` | **4/4 rejetées** (`Error 19`) | `60` |
| 8b | Mutation C : `n <= 64U` → `65U` | **détectée** : 2 échecs + 9614 divergences (identique à la passe 1) | `61` |
| 9 | Débit (100 M triplets) | voir « Mesures » | `70` |

## Diff du C passe 1 → passe 2 (`logs/v2/45-diff-C-passe1-vs-passe2.diff`)
- Signatures : `uint32_t len/cap/*out_len` → **`size_t`** (`SZ.t` en Pulse). Adaptateur `kr_shim_pulse.c` réduit aux tests de `NULL` (19 lignes) : plus de refus `n > UINT32_MAX`, plus de clamp `cap > 64`.
- `memcpy(out, b + 3U, n)` → boucle octet par octet (c'est mon code Pulse : `Pulse.Lib.Array.memcpy_l` ne copie que depuis l'indice 0 ; gcc 13 -O2 n'en refait pas un `memcpy`).
- `for (uint32_t i …)` de `C.Loops.for` → `while (true) { if (!(i < len)) break; … }` ; `Core_Low_fnv_step` (inline) disparaît (fonction `inline_for_extraction`).
- En-tête nommé `Custard.h` (un `.krml` Custard = un seul module `Custard`, d'où `-no-prefix Custard`) ; casts redondants `(size_t)(uint32_t)(uint16_t)(uint32_t)n` ; `F* version: <unknown>` toujours dans l'en-tête.
- `.o` : text 510 o (passe 1) → **357 o** ; même 357 o pour la sortie `--codegen C` directe (53 l., plus lisible, `logs/v2/46`).

## Bugs / contournements : état
| Passe 1 | Passe 2 |
|---|---|
| #1–#4 (wasm, fix, process, Version.ml, .deb) | sans objet : opam installe tout, `make minimal` génère `Version.ml` |
| #5 `noextract` sur la spec | **plus nécessaire** avec racines explicites (testé : `logs/v2/56`) |
| #6 `UInt32.uint_to_t` sans implémentation | toujours : `FStar.Int.Cast` utilisé |
| #7 `C.Loops.for` à extraire | sans objet (boucle `while` Pulse) |
| #8 constantes sans `inline_for_extraction` → globales mutables | **toujours** : sans lui, krml émet `uint32_t Core_Pulse_fnv_prime = 16777619U;` etc. (globales non `const`, `logs/v2/56`) |
| #9 `-D_DEFAULT_SOURCE` | **toujours** nécessaire (`logs/v2/57`) |
| #10 preuves `Seq` | aucun problème rencontré ; deux raffinements à déplacer sur les binders (`#s:erased bytes{SZ.v len <= S.length s}`) pour que les `ensures` soient bien typés |
| nouveau | krmllib non construisible avec F* 2026 (`--cmi`) ; Low* non vérifiable (3 erreurs ci-dessus) |

## Lignes et tailles
| | |
|---|---|
| `Core_Pulse.fst` | 228 lignes (200 non vides ; 87 pour les 3 fonctions Pulse, le reste = spec + lemmes repris de `Core_Low.fst`) ; passe 1 `Core_Low.fst` : 209 |
| C via krml `gen/Custard.c` + `.h` | 71 + 21 (66 non vides) ; C direct `gen-direct/Core_Pulse.c` + `.h` : 53 + 23 |
| Ratio F* / C | 228 / 66 ≈ 3,5 (passe 1 : 3,7) |
| `libcorepulse.a` / `.so` / `_shim.a` | 1 822 / 15 360 / 3 706 o ; symboles `Core_Pulse_{hash,parse,validate}`, aucun appel externe |
| Hors dépôt | opam root 642 Mo, karamel 221 Mo |

## Mesures (gcc -O2, boucle de `core/callbench.c`, 100 M triplets validate+hash+parse, trame de 8 o, `taskset -c 2`)
MESURES_A_REMPLIR

## Fichiers
`Core_Pulse.fst`, `build.sh`, `tests/{test_core_pulse.c,kr_shim_pulse.c,callbench_direct_pulse.c}`, `gen/` (C via krml), `gen-direct/` (C direct), `logs/v2/`, `passe1/` (Low\*, scripts et C de la passe 1, non reconstructibles avec F* 2026).
