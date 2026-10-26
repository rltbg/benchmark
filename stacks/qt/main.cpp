// Qt 6.12 (Qt Quick / QML + C++) implementation of the reference benchmark app (spec v2).
#include <QGuiApplication>
#include <QQmlEngine>
#include <QQuickView>
#include <QQuickWindow>
#include <QSurfaceFormat>
#include "bench.h"

// Repository root: BENCH_ROOT, else the first ancestor of the executable's directory that contains core/benchcore.h.
static QString benchRoot() {
    const QString env = qEnvironmentVariable("BENCH_ROOT");
    if (!env.isEmpty()) return env;
    QDir d(QCoreApplication::applicationDirPath());
    do { if (d.exists("core/benchcore.h")) return d.absolutePath(); } while (d.cdUp());
    return QDir::currentPath();
}

int main(int argc, char **argv) {
    Args a;
    for (int i = 1; i < argc; ++i) {
        const QString k = QString::fromLocal8Bit(argv[i]);
        auto nx = [&] { return i + 1 < argc ? QString::fromLocal8Bit(argv[++i]) : QString(); };
        if (k == "--scenario") a.scenario = nx(); else if (k == "--duration") a.duration = nx().toDouble();
        else if (k == "--out") a.out = nx(); else if (k == "--rate") a.rate = nx().toInt();
        else if (k == "--shapes") a.shapes = nx().toInt(); else if (k == "--assets") a.assets = nx();
        else if (k == "--canvas-impl") a.canvasImpl = nx(); else if (k == "--backend") a.backend = nx();
        else if (k == "--shot") a.shot = nx(); else if (k == "--paced") a.paced = true;
    }
    // QWindow::requestUpdate() docs: update requests are delivered after up to 5 ms unless overridden by the documented
    // QT_QPA_UPDATE_IDLE_TIME environment variable (the Scene Graph docs recommend 0 for benchmarking). --paced keeps Qt's default.
    if (!a.paced && !qEnvironmentVariableIsSet("QT_QPA_UPDATE_IDLE_TIME")) qputenv("QT_QPA_UPDATE_IDLE_TIME", "0");
    // Default: Qt Quick software adaptation (QQuickWindow::setSceneGraphBackend("software"), before the first window).
    // --backend opengl: RHI with OpenGL (Mesa llvmpipe here); swap interval 0 = no vsync blocking (QSurfaceFormat, public).
    if (a.backend.isEmpty()) a.backend = (qEnvironmentVariableIsSet("QT_QUICK_BACKEND") || qEnvironmentVariableIsSet("QSG_RHI_BACKEND")) ? "env" : "software";
    if (a.backend == "software") QQuickWindow::setSceneGraphBackend("software");
    else if (a.backend == "opengl") {
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
        QSurfaceFormat fmt = QSurfaceFormat::defaultFormat(); fmt.setSwapInterval(a.paced ? 1 : 0); QSurfaceFormat::setDefaultFormat(fmt);
    }
    QGuiApplication app(argc, argv);
    if (a.assets.isEmpty()) a.assets = benchRoot() + "/assets";

    Bench bench(a);
    QQuickView view;
    bench.setView(&view);
    view.engine()->setExternalSingletonInstance("Bench", "App", &bench);
    if (a.scenario == "images") { bench.imgProv = new ImgProvider(a.assets); view.engine()->addImageProvider("bench", bench.imgProv); }
    view.setResizeMode(QQuickView::SizeRootObjectToView);
    view.loadFromModule("Bench", "Main");
    if (view.status() != QQuickView::Ready) { for (auto &e : view.errors()) fprintf(stderr, "%s\n", qPrintable(e.toString())); return 2; }
    view.setMinimumSize(QSize(1280, 720)); view.setMaximumSize(QSize(1280, 720));
    view.setGeometry(0, 0, 1280, 720);   // input scenario: the square must be at screen (100,100) under Xvfb (no WM)
    view.setTitle("bench-qt");
    view.show();
    view.requestActivate();               // keyboard focus for the input scenario
    bench.start();
    return app.exec();
}
