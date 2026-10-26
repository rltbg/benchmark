#include "spike.h"
#include "prices.h"
#include <QCoreApplication>
#include <QImage>
#include <QThread>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <vector>
#if defined(Q_OS_ANDROID)
#  include <android/log.h>
#  include <QJniObject>
#  include <QtCore/qcoreapplication_platform.h>
#endif
#if defined(Q_OS_WASM)
#  include <emscripten.h>
#endif
extern "C" {
#include "Custard.h"
}

static const char *kCore = "fstar-pulse-c";

Spike::Spike(QObject *parent) : QObject(parent) {
    m_clock.start();
    QString detail;
    m_selftestOk = selfTest(detail);
    marker(m_selftestOk ? QStringLiteral("SPIKE:selftest ok") : QStringLiteral("SPIKE:selftest fail ") + detail);
    m_banner = QStringLiteral("qt · %1 · cœur=%2 · autotest=%3")
                   .arg(platformName(), kCore, m_selftestOk ? QStringLiteral("OK") : QStringLiteral("ÉCHEC"));
    setInputText(QString());
    // FPS : valeur recalculée chaque seconde ; marqueur seulement quand l'onglet Canvas est actif
    auto *t = new QTimer(this);
    t->setInterval(1000);
    connect(t, &QTimer::timeout, this, [this] {
        const qint64 now = m_clock.elapsed();
        while (!m_frames.empty() && now - m_frames.front() > 1000) m_frames.pop_front();
        m_fps = int(m_frames.size());
        emit fpsChanged();
        if (m_tab == 1) marker(QStringLiteral("SPIKE:fps tab=canvas value=%1").arg(m_fps));
    });
    t->start();
}

QString Spike::platformName() {
#if defined(Q_OS_ANDROID)
    return QStringLiteral("android");
#elif defined(Q_OS_IOS)
    return QStringLiteral("ios");
#elif defined(Q_OS_WASM)
    return QStringLiteral("web");
#elif defined(Q_OS_MACOS)
    return QStringLiteral("macos");
#elif defined(Q_OS_WIN)
    return QStringLiteral("windows");
#else
    return QStringLiteral("linux");
#endif
}

bool Spike::mobile() const {
#if defined(Q_OS_ANDROID) || defined(Q_OS_IOS)
    return true;
#else
    return false;
#endif
}

void Spike::marker(const QString &line) {
    const QByteArray b = line.toUtf8();
#if defined(Q_OS_ANDROID)
    __android_log_write(ANDROID_LOG_INFO, "SPIKE", b.constData());   // logcat
#else
    std::fprintf(stderr, "%s\n", b.constData());                      // stderr (console.error/log du navigateur sur wasm)
    std::fflush(stderr);
#endif
}

bool Spike::wantsAutorun(int argc, char **argv) {
    for (int i = 1; i < argc; ++i) if (!std::strcmp(argv[i], "--autorun")) return true;       // iOS (simctl launch)
    const char *e = std::getenv("SPIKE_AUTORUN");                                               // bureau
    if (e && std::strcmp(e, "1") == 0) return true;
#if defined(Q_OS_WASM)
    return EM_ASM_INT({ return new URLSearchParams(location.search).get('autorun') === '1' ? 1 : 0; }) != 0;   // ?autorun=1
#elif defined(Q_OS_ANDROID)
    // extra d'intent autorun (am start -e autorun 1)
    QJniObject activity = QNativeInterface::QAndroidApplication::context();
    if (activity.isValid()) {
        QJniObject intent = activity.callObjectMethod("getIntent", "()Landroid/content/Intent;");
        if (intent.isValid()) {
            QJniObject key = QJniObject::fromString(QStringLiteral("autorun"));
            return intent.callMethod<jboolean>("hasExtra", "(Ljava/lang/String;)Z", key.object<jstring>());
        }
    }
    return false;
#else
    return false;
#endif
}

// ----- cœur vérifié (C généré par F* Pulse) -----
static QByteArray makeFrame(const QByteArray &payload) {
    const QByteArray p = payload.left(64);                         // tronqué à 64 octets
    QByteArray f;
    f.append(char(0xA5)); f.append(char(0x01)); f.append(char(p.size())); f.append(p);
    return f;
}
static uint32_t hashOf(const QByteArray &b) { return Core_Pulse_hash(reinterpret_cast<uint8_t *>(const_cast<char *>(b.constData())), size_t(b.size())); }
static bool validOf(const QByteArray &b) { return Core_Pulse_validate(reinterpret_cast<uint8_t *>(const_cast<char *>(b.constData())), size_t(b.size())); }

void Spike::setInputText(const QString &t) {
    m_input = t;
    const QByteArray frame = makeFrame(t.toUtf8());
    const bool ok = validOf(frame);
    uint8_t out[64]; size_t n = 0;
    QByteArray payload;
    if (ok && Core_Pulse_parse(reinterpret_cast<uint8_t *>(const_cast<char *>(frame.constData())), size_t(frame.size()), out, sizeof out, &n))
        payload = QByteArray(reinterpret_cast<char *>(out), int(n));
    m_frameHex = QString::fromLatin1(frame.toHex(' '));
    m_valid = ok ? QStringLiteral("true") : QStringLiteral("false");
    m_payload = QString::fromUtf8(payload);
    m_hash = QStringLiteral("%1").arg(hashOf(payload), 8, 16, QLatin1Char('0'));
    emit inputTextChanged();
    emit coreChanged();
}

bool Spike::selfTest(QString &d) {
    struct V { const char *s; uint32_t h; } vec[] = {{"", 0x811C9DC5u}, {"a", 0xE40C292Cu}, {"foobar", 0xBF9CF968u}};
    for (const V &v : vec) {
        const uint32_t h = hashOf(QByteArray(v.s));
        if (h != v.h) { d = QStringLiteral("fnv(%1)=%2").arg(v.s).arg(h, 0, 16); return false; }
    }
    auto frame = [](int n, uint8_t m = 0xA5, uint8_t ver = 1, int declared = -1) {
        QByteArray f; f.append(char(m)); f.append(char(ver)); f.append(char(declared < 0 ? n : declared)); f.append(QByteArray(n, 'x')); return f;
    };
    if (!validOf(frame(0)))  { d = QStringLiteral("frame0 should be valid"); return false; }
    if (!validOf(frame(64))) { d = QStringLiteral("frame64 should be valid"); return false; }
    if (validOf(frame(65)))  { d = QStringLiteral("frame65 should be invalid"); return false; }
    if (validOf(frame(5, 0xA4))) { d = QStringLiteral("bad magic accepted"); return false; }
    if (validOf(frame(5, 0xA5, 2))) { d = QStringLiteral("bad version accepted"); return false; }
    if (validOf(frame(4, 0xA5, 1, 5))) { d = QStringLiteral("inconsistent length accepted"); return false; }
    // aller-retour : trame -> parse -> même charge
    const QByteArray p("héllo"), f = makeFrame(p);
    uint8_t out[64]; size_t n = 0;
    if (!Core_Pulse_parse(reinterpret_cast<uint8_t *>(const_cast<char *>(f.constData())), size_t(f.size()), out, sizeof out, &n)
        || QByteArray(reinterpret_cast<char *>(out), int(n)) != p) { d = QStringLiteral("roundtrip"); return false; }
    return true;
}

// ----- charge 100 ms sur un fil de fond (pendant que le canvas anime) -----
void Spike::heavy() {
    if (m_busy) return;
    m_busy = true; m_trackGap = true; m_maxGap = 0;
    m_heavyText = QStringLiteral("calcul en cours…");
    emit busyChanged();
    marker(QStringLiteral("SPIKE:heavy start"));
    auto work = [] {
        QElapsedTimer t; t.start();
        uint8_t buf[64]; for (int i = 0; i < 64; ++i) buf[i] = uint8_t(i);
        uint32_t acc = 0; qint64 iters = 0;
        while (t.elapsed() < 100) { for (int k = 0; k < 1000; ++k) { buf[0] = uint8_t(iters + k); acc ^= Core_Pulse_hash(buf, 64); } iters += 1000; }
        return std::pair<qint64, uint32_t>{iters, acc};
    };
    auto finish = [this](qint64 iters, uint32_t acc, bool threaded) {
        m_busy = false; m_trackGap = false;
        m_heavyText = QStringLiteral("terminé : %1 hachages (acc=%2)").arg(iters).arg(acc, 0, 16);
        marker(QStringLiteral("SPIKE:heavy done iters=%1 thread=%2 max_frame_gap_ms=%3").arg(iters).arg(threaded ? "worker" : "main").arg(m_maxGap));
        emit busyChanged();
    };
#if QT_CONFIG(thread)
    QThread *th = QThread::create([=] {
        auto r = work();
        QMetaObject::invokeMethod(this, [=] { finish(r.first, r.second, true); }, Qt::QueuedConnection);
    });
    connect(th, &QThread::finished, th, &QObject::deleteLater);
    th->start();
#else
    // wasm monofil : pas de fil possible, le calcul bloque le fil principal (documenté dans NOTES.md)
    auto r = work();
    finish(r.first, r.second, false);
#endif
}

// ----- frames, ready, parcours automatique -----
void Spike::attach(QQuickWindow *w) {
    m_win = w;
    connect(w, &QQuickWindow::frameSwapped, this, &Spike::onFrame);
}

void Spike::onFrame() {
    const qint64 now = m_clock.elapsed();
    m_frames.push_back(now);
    if (m_trackGap && m_lastFrame) m_maxGap = qMax(m_maxGap, now - m_lastFrame);
    m_lastFrame = now;
    if (!m_ready) {
        m_ready = true;
        marker(QStringLiteral("SPIKE:ready platform=%1 stack=qt core=%2").arg(platformName(), kCore));
        if (m_autorun) QTimer::singleShot(500, this, &Spike::startAutorun);
    }
}

void Spike::grab(const QString &name) {
    const QByteArray dir = qgetenv("SPIKE_SHOT_DIR");       // captures de la fenêtre (bureau) ; mobile/web : captures externes
    if (dir.isEmpty() || !m_win) return;
    const QImage img = m_win->grabWindow();
    const QString path = QString::fromLocal8Bit(dir) + QLatin1Char('/') + name + QStringLiteral(".png");
    marker(QStringLiteral("SPIKE:shot %1 %2 %3x%4").arg(name, img.save(path) ? "saved" : "FAILED").arg(img.width()).arg(img.height()));
}

void Spike::startAutorun() {
    marker(QStringLiteral("SPIKE:autorun start"));
    setCurrentTab(0);
    emit scrollDemo();
    QTimer::singleShot(2800, this, [this] { grab(QStringLiteral("tab1-list")); });
    QTimer::singleShot(3000, this, [this] {
        setCurrentTab(1);
        QTimer::singleShot(2800, this, [this] { grab(QStringLiteral("tab2-canvas")); });
        QTimer::singleShot(3000, this, [this] {
            setCurrentTab(2);
            // saisie simulée de "hello", puis charge 100 ms
            const QString word = QStringLiteral("hello");
            for (int i = 1; i <= word.size(); ++i)
                QTimer::singleShot(100 * i, this, [this, word, i] { setInputText(word.left(i)); });
            QTimer::singleShot(700, this, [this] { heavy(); });
            QTimer::singleShot(1700, this, [this] { grab(QStringLiteral("tab3-core")); });
            QTimer::singleShot(2000, this, [this] {
                marker(QStringLiteral("SPIKE:done"));
            });
        });
    });
}
