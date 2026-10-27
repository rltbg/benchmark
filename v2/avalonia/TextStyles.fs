namespace AvaloniaBenchUi

open Avalonia.Markup.Xaml

/// Styles AvaloniaEdit via XAML compile (TextStyles.axaml, x:Class) : un StyleInclude cree en code n'est pas sur en
/// NativeAOT (IL2026 a la publication, puis XamlLoadException "No precompiled XAML found" a l'execution, voir logs/).
/// Type dans un namespace (pas dans le module AvaloniaBench) pour que x:Class le trouve.
type TextStyles() as this =
    inherit Avalonia.Styling.Styles()
    do AvaloniaXamlLoader.Load(this)
