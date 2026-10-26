module Core_Pulse
(* Porte 2, passe 2 : le noyau validate / parse / hash réécrit en Pulse.
   F* v2026.10.25 n'a plus Low* (LowStar.*, FStar.HyperStack.* déplacés dans
   FStarLang/LowStar le 2026-02-23, modèle de tas supprimé le 2026-03-31) :
   Core_Low.fst (passe 1) ne se vérifie plus. Pulse est livré avec F* et
   s'extrait en C par Custard (--codegen KrmlC puis krml, ou --codegen C).
   Trame : [0xA5][version=1][n][payload : n octets], n <= 64.
   La spécification pure (Seq) est celle de Core_Low.fst, inchangée. *)
#lang-pulse
open Pulse
module A   = Pulse.Lib.Array
module R   = Pulse.Lib.Reference
module SZ  = FStar.SizeT
module U8  = FStar.UInt8
module U16 = FStar.UInt16
module U32 = FStar.UInt32
module S   = FStar.Seq
module Cast = FStar.Int.Cast

type bytes = S.seq U8.t

inline_for_extraction noextract
let magic       : U8.t = 0xA5uy
inline_for_extraction noextract
let version     : U8.t = 1uy
noextract
let max_payload : n:nat{n < 256} = 64
noextract
let header_len  : nat = 3

(* ---------------- Spécification pure (Seq) ---------------- *)

noextract
let well_formed (s:bytes) : bool =
  S.length s >= header_len
  && S.index s 0 = magic && S.index s 1 = version
  && U8.v (S.index s 2) <= max_payload
  && S.length s = header_len + U8.v (S.index s 2)

noextract
type payload = p:bytes{S.length p <= max_payload}

noextract
let parse_spec (s:bytes) : option payload =
  if well_formed s then Some (S.slice s header_len (S.length s)) else None

noextract
let encode_spec (p:payload) : bytes =
  S.append (S.seq_of_list [magic; version; U8.uint_to_t (S.length p)]) p

(* FNV-1a 32 bits : h' = (h xor b) * prime  mod 2^32 *)
inline_for_extraction noextract
let fnv_offset : U32.t = 2166136261ul
inline_for_extraction noextract
let fnv_prime  : U32.t = 16777619ul

inline_for_extraction noextract
let fnv_step (h:U32.t) (b:U8.t) : U32.t =
  U32.mul_mod (U32.logxor h (Cast.uint8_to_uint32 b)) fnv_prime

noextract
let rec hash_spec (s:bytes) : Tot U32.t (decreases S.length s) =
  if S.length s = 0 then fnv_offset
  else fnv_step (hash_spec (S.slice s 0 (S.length s - 1))) (S.index s (S.length s - 1))

(* Spec mathématique sur entiers non bornés (aucune hypothèse machine) *)
noextract
let fnv_step_nat (h:nat{h < pow2 32}) (b:nat{b < 256}) : (r:nat{r < pow2 32}) =
  (FStar.UInt.logxor #32 h b * 16777619) % pow2 32

noextract
let rec hash_nat (s:bytes) : Tot (r:nat{r < pow2 32}) (decreases S.length s) =
  if S.length s = 0 then 2166136261
  else fnv_step_nat (hash_nat (S.slice s 0 (S.length s - 1))) (U8.v (S.index s (S.length s - 1)))

(* ---------------- Lemmes purs (identiques à la passe 1) ---------------- *)

let parse_length_bound (s:bytes)
  : Lemma (match parse_spec s with
           | None -> True
           | Some p -> S.length p <= max_payload /\ S.length s == header_len + S.length p
                     /\ S.length s <= header_len + max_payload)
  = ()

let parse_rejects_short (s:bytes)
  : Lemma (requires S.length s < header_len) (ensures None? (parse_spec s)) = ()

let roundtrip (p:payload)
  : Lemma (parse_spec (encode_spec p) == Some p)
  = let e = encode_spec p in
    assert (S.length e == header_len + S.length p);
    assert (S.index e 0 == magic /\ S.index e 1 == version);
    assert (S.index e 2 == U8.uint_to_t (S.length p));
    assert (S.equal (S.slice e header_len (S.length e)) p)

let fnv_step_correct (h:U32.t) (b:U8.t)
  : Lemma (U32.v (fnv_step h b) == fnv_step_nat (U32.v h) (U8.v b)) = ()

let rec hash_correct (s:bytes)
  : Lemma (ensures U32.v (hash_spec s) == hash_nat s) (decreases S.length s)
  = if S.length s = 0 then ()
    else begin
      let s' = S.slice s 0 (S.length s - 1) in
      hash_correct s';
      fnv_step_correct (hash_spec s') (S.index s (S.length s - 1))
    end

let hash_empty () : Lemma (hash_spec S.empty == 0x811C9DC5ul) = ()
let hash_snoc (s:bytes) (x:U8.t)
  : Lemma (hash_spec (S.snoc s x) == fnv_step (hash_spec s) x)
  = assert (S.equal (S.slice (S.snoc s x) 0 (S.length s)) s)

noextract
let sn (s:bytes) (x:U8.t) : bytes = S.snoc s x

let hash_a () : Lemma (hash_spec (sn S.empty 97uy) == 0xE40C292Cul) =
  hash_snoc S.empty 97uy;
  assert_norm (fnv_step fnv_offset 97uy == 0xE40C292Cul)

let hash_foobar ()
  : Lemma (hash_spec (sn (sn (sn (sn (sn (sn S.empty 102uy) 111uy) 111uy) 98uy) 97uy) 114uy) == 0xBF9CF968ul) =
  let s0 = S.empty in
  let s1 = sn s0 102uy in let s2 = sn s1 111uy in let s3 = sn s2 111uy in
  let s4 = sn s3 98uy in let s5 = sn s4 97uy in
  hash_snoc s0 102uy; hash_snoc s1 111uy; hash_snoc s2 111uy;
  hash_snoc s3 98uy; hash_snoc s4 97uy; hash_snoc s5 114uy;
  assert_norm (fnv_step (fnv_step (fnv_step (fnv_step (fnv_step (fnv_step fnv_offset 102uy) 111uy) 111uy) 98uy) 97uy) 114uy == 0xBF9CF968ul)

(* Un pas de la boucle de hash : préfixe de longueur k+1. *)
let hash_prefix_step (s:bytes) (k:nat{k < S.length s})
  : Lemma (hash_spec (S.slice s 0 (k + 1)) == fnv_step (hash_spec (S.slice s 0 k)) (S.index s k))
  = let t = S.slice s 0 (k + 1) in
    assert (S.equal (S.slice t 0 k) (S.slice s 0 k))

(* Vue de l'entrée : les [len] premiers octets. *)
noextract
let view (s:bytes) (len:nat{len <= S.length s}) : bytes = S.slice s 0 len

(* ---------------- Fonctions Pulse (extraites en C) ---------------- *)

/// validate : lit au plus 3 octets (indices < len), jamais hors bornes ; len est un size_t.
fn validate (b:A.array U8.t) (len:SZ.t) (#p:perm) (#s:erased bytes{SZ.v len <= S.length s})
  preserves pts_to b #p s
  returns r:bool
  ensures pure (r <==> well_formed (view s (SZ.v len)))
{
  if (SZ.lt len 3sz) {
    false
  } else {
    let m = b.(0sz);
    let v = b.(1sz);
    let n = b.(2sz);
    (U8.eq m magic && U8.eq v version && U8.lte n 64uy &&
     SZ.eq len (SZ.add 3sz (SZ.uint16_to_sizet (Cast.uint8_to_uint16 n))))
  }
}

/// parse : copie le payload dans [out] (capacité [cap]) et écrit sa longueur dans [out_len].
/// true ssi la trame est bien formée ET le payload tient dans cap ; sinon rien n'est écrit.
/// La disjonction b / out découle de la séparation (permission pleine sur out).
fn parse (b:A.array U8.t) (len:SZ.t) (out:A.array U8.t) (cap:SZ.t) (out_len:R.ref SZ.t)
  (#p:perm) (#s:erased bytes{SZ.v len <= S.length s}) (#so:erased bytes{S.length so == SZ.v cap}) (#ol:erased SZ.t)
  preserves pts_to b #p s
  requires pts_to out so ** pts_to out_len ol
  returns r:bool
  ensures exists* so' ol'. pts_to out so' ** pts_to out_len ol' ** pure (
    S.length so' == S.length so /\
    (r <==> (Some? (parse_spec (view s (SZ.v len))) /\
             S.length (Some?.v (parse_spec (view s (SZ.v len)))) <= SZ.v cap)) /\
    (r ==> (SZ.v ol' == S.length (Some?.v (parse_spec (view s (SZ.v len)))) /\
            SZ.v ol' <= max_payload /\ SZ.v ol' <= SZ.v cap /\
            S.equal (S.slice so' 0 (SZ.v ol')) (Some?.v (parse_spec (view s (SZ.v len)))))) /\
    (not r ==> (so' == reveal so /\ ol' == reveal ol)))
{
  A.pts_to_len b;
  A.pts_to_len out;
  let ok = validate b len;
  if (not ok) {
    false
  } else {
    let n = SZ.sub len 3sz;
    if (SZ.gt n cap) {
      false
    } else {
      let mut i = 0sz;
      while (SZ.lt !i n)
      invariant exists* vi so1.
        pts_to i vi ** pts_to out so1 ** pts_to b #p s ** pure (
          SZ.v vi <= SZ.v n /\ S.length so1 == S.length so /\
          (forall (k:nat). k < SZ.v vi ==> S.index so1 k == S.index s (3 + k)))
      decreases (SZ.v n - SZ.v !i)
      {
        let vi = !i;
        let x = b.(SZ.add 3sz vi);
        out.(vi) <- x;
        i := SZ.add vi 1sz;
      };
      out_len := n;
      true
    }
  }
}

/// hash FNV-1a 32 bits des [len] premiers octets ; total, sans débordement (mul_mod).
fn hash (b:A.array U8.t) (len:SZ.t) (#p:perm) (#s:erased bytes{SZ.v len <= S.length s})
  preserves pts_to b #p s
  returns r:U32.t
  ensures pure (r == hash_spec (view s (SZ.v len)) /\ U32.v r == hash_nat (view s (SZ.v len)))
{
  A.pts_to_len b;
  let mut i = 0sz;
  let mut h = fnv_offset;
  while (SZ.lt !i len)
  invariant exists* vi vh.
    pts_to i vi ** pts_to h vh ** pts_to b #p s ** pure (
      SZ.v vi <= SZ.v len /\ vh == hash_spec (S.slice s 0 (SZ.v vi)))
  decreases (SZ.v len - SZ.v !i)
  {
    let vi = !i;
    let x = b.(vi);
    let vh = !h;
    hash_prefix_step s (SZ.v vi);
    h := fnv_step vh x;
    i := SZ.add vi 1sz;
  };
  hash_correct (view s (SZ.v len));
  !h
}
