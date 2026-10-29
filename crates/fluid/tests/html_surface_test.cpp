#include "html_surface.h"
#include <QQuickItem>
#include <QQuickWindow>
#include <QtTest>

class HtmlSurfaceTest : public QObject {
    Q_OBJECT
private slots:
    void repeatedVisualParentTeardownReleasesHandlers() {
        QQuickWindow window;
        QQuickItem first(window.contentItem()), second(window.contentItem());
        bool baseWindowChange=false;
        auto probe=new HtmlSurface(&first);
        connect(probe,&QQuickItem::windowChanged,&window,[probe,&baseWindowChange](QQuickWindow *host){
            if(!host)baseWindowChange=qobject_cast<HtmlSurface *>(probe)==nullptr;
        });
        delete probe;
        QVERIFY(baseWindowChange); // signal arrives after the derived destructor
        for (int cycle = 0; cycle < 1000; ++cycle) {
            auto host = new HtmlSurface(&first);
            host->setParentItem(&second);
            host->setWidth(400 + cycle % 10);
            delete host;
            first.setWidth(600 + cycle % 10);
            second.setHeight(500 + cycle % 10);
            QVERIFY(first.childItems().isEmpty());
            QVERIFY(second.childItems().isEmpty());
        }
        QCOMPARE(HtmlSurface::liveNativeSurfaces(), 0);
        QCoreApplication::processEvents();
    }
};
QTEST_MAIN(HtmlSurfaceTest)
#include "html_surface_test.moc"
