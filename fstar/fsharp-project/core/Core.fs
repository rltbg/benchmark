#light "off"
module Core

type bytes =
FStar_UInt8.t Prims.list


let magic : FStar_UInt8.t = (FStar_UInt8.uint_to_t (Prims.parse_int "165"))


let version : FStar_UInt8.t = (FStar_UInt8.uint_to_t (Prims.parse_int "1"))


let max_payload : Prims.nat = (Prims.parse_int "64")


let header_len : Prims.nat = (Prims.parse_int "3")


type payload =
bytes

type msg =
{payload : payload}


let __proj__Mkmsg__item__payload : msg  ->  payload = (fun ( projectee  :  msg ) -> (match (projectee) with
| {payload = payload1} -> begin
payload1
end))


let validate : bytes  ->  Prims.bool = (fun ( b  :  bytes ) -> (match (b) with
| (m)::(v)::(n)::rest -> begin
((((FStar_UInt8.eq m magic) && (FStar_UInt8.eq v version)) && ((FStar_UInt8.v n) <= max_payload)) && (Prims.op_Equality (FStar_List_Tot_Base.length rest) (FStar_UInt8.v n)))
end
| uu___ -> begin
false
end))


let parse : bytes  ->  msg FStar_Pervasives_Native.option = (fun ( b  :  bytes ) -> (match (b) with
| (m)::(v)::(n)::rest -> begin
(match (((((FStar_UInt8.eq m magic) && (FStar_UInt8.eq v version)) && ((FStar_UInt8.v n) <= max_payload)) && (Prims.op_Equality (FStar_List_Tot_Base.length rest) (FStar_UInt8.v n)))) with
| true -> begin
FStar_Pervasives_Native.Some ({payload = rest})
end
| uu___ -> begin
FStar_Pervasives_Native.None
end)
end
| uu___ -> begin
FStar_Pervasives_Native.None
end))


let encode : msg  ->  bytes = (fun ( m  :  msg ) -> (

let n = (FStar_UInt8.uint_to_t (FStar_List_Tot_Base.length m.payload))
in (magic)::(version)::(n)::m.payload))


let checked_add : FStar_UInt32.t  ->  FStar_UInt32.t  ->  FStar_UInt32.t FStar_Pervasives_Native.option = (fun ( a  :  FStar_UInt32.t ) ( b  :  FStar_UInt32.t ) -> (

let s = ((FStar_UInt32.v a) + (FStar_UInt32.v b))
in (match ((s < (Prims.pow2 (Prims.parse_int "32")))) with
| true -> begin
FStar_Pervasives_Native.Some ((FStar_UInt32.add a b))
end
| uu___ -> begin
FStar_Pervasives_Native.None
end)))


let fnv_offset : FStar_UInt32.t = (FStar_UInt32.uint_to_t ((Prims.parse_int "2166136261")))


let fnv_prime : FStar_UInt32.t = (FStar_UInt32.uint_to_t ((Prims.parse_int "16777619")))


let fnv_step : FStar_UInt32.t  ->  FStar_UInt8.t  ->  FStar_UInt32.t = (fun ( h  :  FStar_UInt32.t ) ( b  :  FStar_UInt8.t ) -> (FStar_UInt32.mul_mod (FStar_UInt32.logxor h (FStar_UInt32.uint_to_t (FStar_UInt8.v b))) fnv_prime))


let rec fnv_fold : FStar_UInt32.t  ->  bytes  ->  FStar_UInt32.t = (fun ( h  :  FStar_UInt32.t ) ( b  :  bytes ) -> (match (b) with
| [] -> begin
h
end
| (x)::xs -> begin
(fnv_fold (fnv_step h x) xs)
end))


let hash : bytes  ->  FStar_UInt32.t = (fun ( b  :  bytes ) -> (fnv_fold fnv_offset b))


let hash_frame : bytes  ->  FStar_UInt32.t FStar_Pervasives_Native.option = (fun ( b  :  bytes ) -> (match ((parse b)) with
| FStar_Pervasives_Native.None -> begin
FStar_Pervasives_Native.None
end
| FStar_Pervasives_Native.Some (m) -> begin
FStar_Pervasives_Native.Some ((hash m.payload))
end))




