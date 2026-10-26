#pragma once
#include <QObject>
#include <QQuickWindow>
#include <QElapsedTimer>
#include <QTimer>
#include <QString>
#include <deque>
#include <QtQml/qqmlregistration.h>

// Contrôleur unique (singleton QML) : bandeau, autotest, marqueurs SPIKE:, FPS, cœur, charge 100 ms, parcours automatique.
class Spike : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    QML_UNCREATABLE("fourni par C++ (setExternalSingletonInstance)")
    Q_PROPERTY(QString banner READ banner NOTIFY bannerChanged)
    Q_PROPERTY(int currentTab READ currentTab WRITE setCurrentTab NOTIFY currentTabChanged)
    Q_PROPERTY(bool mobile READ mobile CONSTANT)
    Q_PROPERTY(int fps READ fps NOTIFY fpsChanged)
    Q_PROPERTY(QString inputText READ inputText WRITE setInputText NOTIFY inputTextChanged)
    Q_PROPERTY(QString frameHex READ frameHex NOTIFY coreChanged)
    Q_PROPERTY(QString validText READ validText NOTIFY coreChanged)
    Q_PROPERTY(QString payloadText READ payloadText NOTIFY coreChanged)
    Q_PROPERTY(QString hashHex READ hashHex NOTIFY coreChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString heavyText READ heavyText NOTIFY busyChanged)
public:
    explicit Spike(QObject *parent = nullptr);
    static void marker(const QString &line);           // une ligne "SPIKE:..." sur la sortie de journal de la plate-forme
    static QString platformName();
    static bool wantsAutorun(int argc, char **argv);
    void attach(QQuickWindow *w);                      // suit les frames (ready, FPS)
    void setAutorun(bool on) { m_autorun = on; }
    QString banner() const { return m_banner; }
    int currentTab() const { return m_tab; }
    void setCurrentTab(int t) { if (t != m_tab) { m_tab = t; emit currentTabChanged(); } }
    bool mobile() const;
    int fps() const { return m_fps; }
    QString inputText() const { return m_input; }
    void setInputText(const QString &t);
    QString frameHex() const { return m_frameHex; }
    QString validText() const { return m_valid; }
    QString payloadText() const { return m_payload; }
    QString hashHex() const { return m_hash; }
    bool busy() const { return m_busy; }
    QString heavyText() const { return m_heavyText; }
    Q_INVOKABLE void heavy();                          // boucle de calcul de 100 ms hors du fil de rendu
signals:
    void bannerChanged();
    void currentTabChanged();
    void fpsChanged();
    void inputTextChanged();
    void coreChanged();
    void busyChanged();
    void scrollDemo();                                 // le QML fait défiler la liste
private:
    bool selfTest(QString &detail);
    void onFrame();
    void startAutorun();
    void grab(const QString &name);
    QQuickWindow *m_win = nullptr;
    QString m_banner, m_input, m_frameHex, m_valid, m_payload, m_hash, m_heavyText;
    int m_tab = 0, m_fps = 0;
    bool m_selftestOk = false, m_ready = false, m_autorun = false, m_busy = false;
    std::deque<qint64> m_frames;                       // horodatages (ms) des dernières frames
    QElapsedTimer m_clock;
    qint64 m_lastFrame = 0, m_maxGap = 0;
    bool m_trackGap = false;
};
