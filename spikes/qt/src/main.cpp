#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QQmlContext>
#include <QtQml/qqml.h>
#include <QSurfaceFormat>
#include "spike.h"
#include "prices.h"

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("qtspike"));
    // En CI sans GPU : QT_QUICK_BACKEND=software (documenté : https://doc.qt.io/qt-6/qtquick-visualcanvas-adaptations-software.html)
    auto *spike = new Spike;
    spike->setAutorun(Spike::wantsAutorun(argc, argv));
    QQmlApplicationEngine engine;
    engine.setExternalSingletonInstance("Spike", "Spike", spike);   // Qt 6.12 : singleton QML fourni par C++
    engine.loadFromModule("Spike", "Main");
    if (engine.rootObjects().isEmpty()) { Spike::marker(QStringLiteral("SPIKE:error no root object")); return 1; }
    auto *win = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (!win) { Spike::marker(QStringLiteral("SPIKE:error root is not a window")); return 1; }
    spike->attach(win);
    return app.exec();
}
