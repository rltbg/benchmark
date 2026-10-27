module Core
(* Validateur / parseur d'octets + hash FNV-1a 32 bits, avec spec et lemmes.
   Format d'une trame : [0xA5][version=1][n][payload : n octets], n <= 64. *)
open FStar.Mul
module U8  = FStar.UInt8
module U32 = FStar.UInt32
module L   = FStar.List.Tot.Base

type bytes = list U8.t

let magic       : U8.t = U8.uint_to_t 165
let version     : U8.t = U8.uint_to_t 1
let max_payload : n:nat{n < 256} = 64
let header_len  : nat = 3

(* ---------- Spécification de la trame ---------- *)

type payload = p:bytes{L.length p <= max_payload}
type msg = { payload : payload }

(* Prédicat de bonne formation, côté spec (propositionnel). *)
noextract
let well_formed (b:bytes) : prop =
  match b with
  | m :: v :: n :: rest ->
      m == magic /\ v == version /\ U8.v n <= max_payload /\ L.length rest == U8.v n
  | _ -> False

(* ---------- validate ---------- *)

let validate (b:bytes) : r:bool{r <==> well_formed b} =
  match b with
  | m :: v :: n :: rest ->
      U8.eq m magic && U8.eq v version
      && U8.v n <= max_payload && L.length rest = U8.v n
  | _ -> false

(* ---------- parse ---------- *)

let parse (b:bytes) : option msg =
  match b with
  | m :: v :: n :: rest ->
      if U8.eq m magic && U8.eq v version
         && U8.v n <= max_payload && L.length rest = U8.v n
      then Some ({ payload = rest })
      else None
  | _ -> None

(* Lemme 1 : borne de longueur. Toute trame acceptée a un payload <= 64
   et une longueur totale <= header_len + max_payload (= 67). *)
let parse_length_bound (b:bytes)
  : Lemma (match parse b with
           | None -> True
           | Some m -> L.length m.payload <= max_payload
                    /\ L.length b == header_len + L.length m.payload
                    /\ L.length b <= header_len + max_payload)
  = ()

(* Lemme 2 : parse et validate sont d'accord. *)
let parse_validate_iff (b:bytes)
  : Lemma (Some? (parse b) <==> validate b)
  = ()

(* Lemme 3 : parse ne lit jamais hors bornes — toute trame de moins de 3
   octets ou de longueur incohérente est rejetée. *)
let parse_rejects_short (b:bytes)
  : Lemma (requires L.length b < header_len)
          (ensures  None? (parse b))
  = ()

let parse_rejects_bad_len (b:bytes)
  : Lemma (requires (match b with
                     | _ :: _ :: n :: rest -> L.length rest <> U8.v n
                     | _ -> False))
          (ensures  None? (parse b))
  = ()

(* ---------- encode (inverse de parse) ---------- *)

(* L'octet de longueur est produit par uint_to_t, dont la précondition
   (< 256) est établie par le raffinement de [payload] : pas de débordement. *)
let encode (m:msg) : bytes =
  let n : U8.t = U8.uint_to_t (L.length m.payload) in
  magic :: version :: n :: m.payload

let encode_length (m:msg)
  : Lemma (L.length (encode m) == header_len + L.length m.payload
        /\ L.length (encode m) <= header_len + max_payload)
  = ()

(* Lemme 4 : aller-retour. *)
let roundtrip (m:msg)
  : Lemma (validate (encode m) /\ parse (encode m) == Some m)
  = ()

(* ---------- Arithmétique contrôlée (somme de longueurs) ---------- *)

(* Addition 32 bits qui refuse le débordement : la précondition de
   U32.add est prouvée dans la branche vraie, jamais supposée. *)
let checked_add (a b:U32.t) : r:option U32.t{
    (match r with
     | Some c -> U32.v c == U32.v a + U32.v b
     | None   -> U32.v a + U32.v b >= pow2 32)}
  = let s = U32.v a + U32.v b in
    if s < pow2 32 then Some (U32.add a b) else None

(* Lemme 5 : deux trames bornées ne débordent jamais un compteur 32 bits. *)
let two_frames_fit (a b:msg)
  : Lemma ((L.length (encode a) + L.length (encode b)) < pow2 32)
  = assert_norm (pow2 32 == 4294967296)

(* ---------- FNV-1a 32 bits ---------- *)

let fnv_offset : U32.t = 2166136261ul
let fnv_prime  : U32.t = 16777619ul

(* Étape : h' = (h xor b) * prime  mod 2^32. mul_mod est total : le
   débordement est le comportement voulu et défini. *)
let fnv_step (h:U32.t) (b:U8.t) : U32.t =
  U32.mul_mod (U32.logxor h (FStar.UInt32.uint_to_t (U8.v b))) fnv_prime

let rec fnv_fold (h:U32.t) (b:bytes) : Tot U32.t (decreases b) =
  match b with
  | [] -> h
  | x :: xs -> fnv_fold (fnv_step h x) xs

let hash (b:bytes) : U32.t = fnv_fold fnv_offset b

(* Spec mathématique, sur entiers non bornés, sans machine words. *)
noextract
let fnv_step_spec (h:nat{h < pow2 32}) (b:nat{b < 256}) : (r:nat{r < pow2 32}) =
  let x = FStar.UInt.logxor #32 h b in
  (x * 16777619) % pow2 32

noextract
let rec fnv_fold_spec (h:nat{h < pow2 32}) (b:list nat{forall (x:nat). L.mem x b ==> x < 256})
  : Tot (r:nat{r < pow2 32}) (decreases b) =
  match b with
  | [] -> h
  | x :: xs -> fnv_fold_spec (fnv_step_spec h x) xs

noextract
let rec to_nats (b:bytes) : (l:list nat{forall (x:nat). L.mem x l ==> x < 256}) =
  match b with
  | [] -> []
  | x :: xs -> U8.v x :: to_nats xs

let fnv_step_correct (h:U32.t) (b:U8.t)
  : Lemma (U32.v (fnv_step h b) == fnv_step_spec (U32.v h) (U8.v b))
  = ()

(* Lemme 6 : le hash machine (arithmétique modulaire) coïncide avec la
   spec mathématique : aucune perte, aucun débordement non modélisé. *)
let rec hash_correct (h:U32.t) (b:bytes)
  : Lemma (ensures U32.v (fnv_fold h b) == fnv_fold_spec (U32.v h) (to_nats b))
          (decreases b)
  = match b with
    | [] -> ()
    | x :: xs -> fnv_step_correct h x; hash_correct (fnv_step h x) xs

(* Lemme 7 : hash incrémental — hacher une concaténation = enchaîner. *)
let rec fold_append (h:U32.t) (a b:bytes)
  : Lemma (ensures fnv_fold h (L.append a b) == fnv_fold (fnv_fold h a) b)
          (decreases a)
  = match a with
    | [] -> ()
    | x :: xs -> fold_append (fnv_step h x) xs b

(* Vecteurs de test connus (FNV-1a 32) : "" -> 0x811C9DC5, "a" -> 0xE40C292C. *)
let hash_empty () : Lemma (hash [] == 0x811C9DC5ul) = ()
let hash_a () : Lemma (hash [97uy] == 0xE40C292Cul) = assert_norm (U32.v (hash [97uy]) == 0xE40C292C)

(* Point d'entrée applicatif : hash du payload d'une trame valide. *)
let hash_frame (b:bytes) : option U32.t =
  match parse b with
  | None -> None
  | Some m -> Some (hash m.payload)
