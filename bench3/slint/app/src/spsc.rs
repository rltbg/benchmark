// File SPSC sans verrou (un producteur, un consommateur) à capacité fixe, pour les flux natif -> UI.
// Aucun appel à Slint depuis le thread producteur : le fil UI draine la file à chaque frame.
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct Spsc<T: Copy> {
    buf: Box<[UnsafeCell<T>]>,
    mask: usize,
    head: AtomicUsize, // prochain à lire (consommateur)
    tail: AtomicUsize, // prochain à écrire (producteur)
}
unsafe impl<T: Copy + Send> Send for Spsc<T> {}
unsafe impl<T: Copy + Send> Sync for Spsc<T> {}

impl<T: Copy + Default> Spsc<T> {
    /// capacité arrondie à la puissance de 2 supérieure
    pub fn new(cap: usize) -> Arc<Self> {
        let cap = cap.next_power_of_two();
        let buf: Vec<UnsafeCell<T>> = (0..cap).map(|_| UnsafeCell::new(T::default())).collect();
        Arc::new(Spsc { buf: buf.into_boxed_slice(), mask: cap - 1, head: AtomicUsize::new(0), tail: AtomicUsize::new(0) })
    }
    pub fn capacity(&self) -> usize { self.mask + 1 }
    /// producteur uniquement. false si pleine.
    #[inline]
    pub fn push(&self, v: T) -> bool {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t.wrapping_sub(h) > self.mask {
            return false;
        }
        unsafe { *self.buf[t & self.mask].get() = v };
        self.tail.store(t.wrapping_add(1), Ordering::Release);
        true
    }
    /// consommateur uniquement : lit au plus `max` éléments, les passe à `f`, retourne le nombre lu
    pub fn drain(&self, max: usize, mut f: impl FnMut(T)) -> usize {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        let n = t.wrapping_sub(h).min(max);
        for k in 0..n {
            f(unsafe { *self.buf[h.wrapping_add(k) & self.mask].get() });
        }
        self.head.store(h.wrapping_add(n), Ordering::Release);
        n
    }
    pub fn len(&self) -> usize {
        self.tail.load(Ordering::Acquire).wrapping_sub(self.head.load(Ordering::Acquire))
    }
}
