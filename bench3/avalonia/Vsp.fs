module Vsp
// Voie "vsp" : ItemsControl + VirtualizingStackPanel (chemin standard de la doc Avalonia) dans un ScrollViewer.
// Le panneau estime l'extent des elements non mesures (hauteurs variables) : l'extent n'est pas exact (voir NOTES).

open System
open System.Collections
open System.Collections.Generic
open System.Collections.ObjectModel
open System.Collections.Specialized
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.Templates
open Avalonia.Media
open Avalonia.VisualTree
open Util
open ListsCore

/// Liste virtuelle en lecture : O(1) a la construction (pas de materialisation de N elements), INotifyCollectionChanged groupe (Reset par lot)
[<AllowNullLiteral>]
type StoreList(store0: RowStore) =
    let mutable store = store0
    let ev = Event<NotifyCollectionChangedEventHandler, NotifyCollectionChangedEventArgs>()
    static let reset = NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Reset)
    member _.Store with get () = store and set (v: RowStore) = store <- v
    member this.NotifyReset() = ev.Trigger(this, reset)
    interface INotifyCollectionChanged with
        [<CLIEvent>]
        member _.CollectionChanged = ev.Publish
    interface IList with
        member _.Add(_) = raise (NotSupportedException())
        member _.Clear() = raise (NotSupportedException())
        member _.Contains(_) = false
        member _.IndexOf(_) = -1
        member _.Insert(_, _) = raise (NotSupportedException())
        member _.Remove(_) = raise (NotSupportedException())
        member _.RemoveAt(_) = raise (NotSupportedException())
        member _.IsFixedSize = true
        member _.IsReadOnly = true
        member _.Item
            with get (i: int) : obj = box (RowItem(store.Id i, store.Height i, store.Price i))
            and set (_: int) (_: obj) = raise (NotSupportedException())
        member _.CopyTo(_, _) = raise (NotSupportedException())
        member _.Count = store.Count
        member _.IsSynchronized = false
        member _.SyncRoot = null
        member _.GetEnumerator() : IEnumerator =
            (Seq.init store.Count (fun i -> box (RowItem(store.Id i, store.Height i, store.Price i)))).GetEnumerator() :> IEnumerator

type VspListView(source: IEnumerable, dense: bool, np: int, recycle: bool, cacheLength: float) =
    let items =
        ItemsControl(ItemsSource = source,
                     ItemsPanel = FuncTemplate<Panel>(fun () -> VirtualizingStackPanel(CacheLength = cacheLength) :> Panel),
                     ItemTemplate = FuncDataTemplate<RowItem>((fun _ _ -> RowView(dense, np) :> Control), recycle))
    let sv = ScrollViewer(Content = items, HorizontalScrollBarVisibility = Primitives.ScrollBarVisibility.Disabled,
                          VerticalScrollBarVisibility = Primitives.ScrollBarVisibility.Hidden, Background = Brushes.White)
    member _.Control : Control = sv :> Control
    member _.Items = items
    member _.Scroll = sv
    member _.Source with get () = items.ItemsSource and set (v: IEnumerable) = items.ItemsSource <- v
    member _.FirstRealized =
        match items.ItemsPanelRoot with
        | :? VirtualizingStackPanel as p -> p.FirstRealizedIndex
        | _ -> 0
    member _.MaxOffset = max 0.0 (sv.Extent.Height - sv.Viewport.Height)
