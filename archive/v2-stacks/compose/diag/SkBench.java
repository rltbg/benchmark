import org.jetbrains.skia.*;
import java.nio.file.*;
/** Micro-bench Skia raster pur (sans Compose) : 1 frame = 63 vignettes 128x128 de la grille, canvas mis à l'échelle s. */
public class SkBench {
  public static void main(String[] a) throws Exception {
    Image src = Image.Companion.makeFromEncoded(Files.readAllBytes(java.nio.file.Path.of(a[0])));
    Surface th = Surface.Companion.makeRasterN32Premul(128, 128);
    th.getCanvas().drawImageRect(src, Rect.Companion.makeWH(256,256), Rect.Companion.makeWH(128,128), SamplingMode.Companion.getLINEAR(), null, true);
    Image img = th.makeImageSnapshot();
    Surface s = Surface.Companion.makeRasterN32Premul(1280, 720);
    Canvas c = s.getCanvas();
    for (boolean strict : new boolean[]{true,false}) for (String mode : new String[]{"LINEAR","NEAREST"}) for (float sc : new float[]{1f, 3f}) {
      SamplingMode sm = mode.equals("LINEAR") ? SamplingMode.Companion.getLINEAR() : SamplingMode.Companion.getDEFAULT();
      long best = Long.MAX_VALUE;
      for (int rep = 0; rep < 5; rep++) {
        long t0 = System.nanoTime();
        for (int f = 0; f < 40; f++) {
          c.clear(0xFF1E1E24);
          c.save(); c.translate(640, 360); c.scale(sc, sc); c.translate(-640, -360 + (f % 2));
          for (int r = 0; r < 7; r++) for (int k = 0; k < 9; k++) {
            float x = 40 + k * 134, y = 28 + r * 134;
            c.drawImageRect(img, Rect.Companion.makeWH(128,128), Rect.Companion.makeXYWH(x, y, 128, 128), sm, null, strict);
          }
          c.restore();
        }
        best = Math.min(best, (System.nanoTime() - t0) / 40);
      }
      System.out.printf("strict=%b %s scale %.0f : %.2f ms/frame%n", strict, mode, sc, best / 1e6);
    }
  }
}
