#include "motion_smoke.h"
#include "bridge.h"
#include "motion.h"
#include <QQmlEngine>
#include <QCoreApplication>
#include <QJsonDocument>
#include <QLineF>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSaveFile>
#include <QTimer>
#include <cmath>

namespace {
QQuickItem *visualItem(QQuickItem *parent,const QString &name){
    if(parent->objectName()==name&&!parent->property("pooled").toBool())return parent;
    for(auto child:parent->childItems())if(auto found=visualItem(child,name))return found;
    return nullptr;
}
}
MotionSmoke::MotionSmoke(Bridge *bridge,QQuickWindow *window)
    : QObject(window),bridge(bridge),window(window) {
    const int previewWidth=qEnvironmentVariableIntValue("LUMA_PREVIEW_WIDTH");
    if(previewWidth>=760&&previewWidth<=3840)window->setWidth(previewWidth);
    connect(bridge,&Bridge::ready,this,[this]{qInfo()<<"MOTION startup: synthetic engine ready";if(!started){started=true;QTimer::singleShot(300,this,[this]{begin();});}});
    connect(window,&QQuickWindow::afterFrameEnd,this,[this]{if(elapsed.isValid()){++totalFrames;if(frames.size()<4096)frames.append(elapsed.elapsed());}},Qt::QueuedConnection);
}
void MotionSmoke::check(const QString &name,bool passed) {
    checks.append(QJsonObject{{"check",name},{"passed",passed},{"atMs",elapsed.isValid()?elapsed.elapsed():0},{"exposed",window->isExposed()},{"visible",window->isVisible()},{"minimized",window->visibility()==QWindow::Minimized}});
    failed|=!passed;
    qInfo().noquote()<<"MOTION check"<<(elapsed.isValid()?elapsed.elapsed():0)<<name<<passed;
}
void MotionSmoke::open(int index,int modifiers) {
    QMetaObject::invokeMethod(inbox,"openIndex",Q_ARG(QVariant,index),Q_ARG(QVariant,modifiers));
}
void MotionSmoke::begin() {
    elapsed.start();
    check("native animation test has an exposed window",window->isExposed());
    if(!window->isExposed()){finish();return;}
    navigation=window->findChild<QQuickItem*>("navigation");
    inbox=window->findChild<QQuickItem*>("inboxPane");
    morph=window->findChild<QQuickItem*>("rowMorph");
    if(!navigation||!inbox||!morph){check("native scene loaded",false);finish();return;}
    check("unified folders start collapsed",!navigation->property("unifiedOpen").toBool());
    check("account folders start collapsed",navigation->property("accountOpen").toString().isEmpty());
    check("bounded engine preview loaded",bridge->inbox()->rowCount()==8);
    open(1);
    check("opening email starts a shared projection",morph->property("active").toBool()&&morph->property("opening").toBool());
    QTimer::singleShot(70,this,[this]{
        const QPointF before=morph->position();
        open(3);
        check("rapid selection preserves projection position",QLineF(before,morph->position()).length()<.1);
        check("rapid selection retargets the requested email",morph->property("row").toMap().value("key").toMap().value("id")=="preview3");
    });
    QTimer::singleShot(1000,this,[this]{
        check("opening spring reaches rest",!morph->property("active").toBool()&&!morph->property("moving").toBool());
        check("reader shows latest selection",bridge->reader().value("key").toMap().value("id")=="preview3");
        auto subject=visualItem(window->contentItem(),"readerSubject");
        check("mail headers render as literal text",subject&&subject->property("text").toString()==bridge->reader().value("subject").toString().toHtmlEscaped());
        auto projected=visualItem(window->contentItem(),"projectedAvatar");
        auto actual=visualItem(window->contentItem(),"readerAvatar-0");
        check("avatar handoff matches reader geometry",projected&&actual&&QLineF(projected->mapToScene(QPointF()),actual->mapToScene(QPointF())).length()<1&&std::abs(projected->width()-actual->width())<.5);
        const qreal compact=window->width()<1000?0:qMin(bridge->settings().value("list_width",450).toReal(),qMax(qreal(0),window->width()-navigation->width()-380));
        check("compact inbox keeps its responsive bounds",std::abs(inbox->width()-compact)<.5);
        bridge->closeReader();
        check("back reverses the shared projection",morph->property("active").toBool()&&!morph->property("opening").toBool());
    });
    QTimer::singleShot(2000,this,[this]{
        check("return spring reaches rest",!morph->property("active").toBool());
        check("inbox expands after closing reader",std::abs(inbox->width()-(window->width()-navigation->width()))<.5);
        navigation->setProperty("accountOpen","preview");
        bridge->setView("inbox","preview");
    });
    QTimer::singleShot(2800,this,[this]{
        check("account-specific folders remain available",navigation->property("accountOpen")=="preview"&&bridge->account()=="preview");
        open(1);
        bridge->selectRow(0,Qt::ControlModifier,0,0,0,0);
        bridge->selectRow(3,Qt::ShiftModifier,0,0,0,0);
        check("Ctrl then Shift selects the full range",bridge->selectionCount()==4);
        bridge->clearSelection();
        bridge->send("settingsUpdate",{{"reduced_motion",true}});
    });
    QTimer::singleShot(3500,this,[this]{
        check("reduced-motion preference reached the engine",bridge->settings().value("reduced_motion").toBool());
        open(2);
        check("reduced motion opens without projection",!morph->property("active").toBool());
        const qreal compact=window->width()<1000?0:qMin(bridge->settings().value("list_width",450).toReal(),qMax(qreal(0),window->width()-navigation->width()-380));
        check("reduced motion settles pane immediately",std::abs(inbox->width()-compact)<.5);
        QTimer::singleShot(200,this,[this]{
            check("received body belongs to selected reader",bridge->messages()->at(0).value("id")=="preview2"&&bridge->messages()->at(0).value("text").isValid());
            bridge->closeReader();
            bridge->send("settingsUpdate",{{"reduced_motion",false}});
            archiveWhenReady();
        });
    });
}
void MotionSmoke::archiveWhenReady(int attempts){
    if(finished)return;
    auto row=visualItem(window->contentItem(),"mailRow-preview4");
    // The preceding test changed reduced motion asynchronously. A coarse
    // single-shot timer can fire before that command's acknowledgement.
    // Observe the actual preference and active delegate before requesting the
    // removal; never skip the animation/row-removal assertions below.
    if(bridge->settings().value("reduced_motion").toBool()||!row||!row->isVisible()||row->width()<=0){
        if(attempts<=0){check("archive fixture receives the motion preference and active row",false);finish();return;}
        QTimer::singleShot(25,this,[this,attempts]{archiveWhenReady(attempts-1);});return;
    }
    const auto initial=row->height();
    connect(row,&QQuickItem::heightChanged,this,[this,row=QPointer<QQuickItem>(row),initial]{
        if(row&&row->property("removing").toBool()&&row->height()>0&&row->height()<initial)removalAnimated=true;
    });
    bridge->mutate("archive",bridge->inbox()->at(4));
    QTimer::singleShot(900,this,[this]{
        check("server-confirmed archive animates row height",removalAnimated);
        check("archived row leaves the list without a refresh",bridge->inbox()->rowCount()==7);
        check("mail commands produced no errors",bridge->error().isEmpty());
        auto composer=visualItem(window->contentItem(),"composer");
        if(!composer){check("composer scene loaded",false);finish();return;}
        QMetaObject::invokeMethod(composer,"open",Q_ARG(QVariant,"new"));
        check("new composer starts compact",!composer->property("expanded").toBool());
        check("compose button starts a bounds morph",composer->property("moving").toBool());
        whenStill(composer,[this,composer]{
            check("compact composer reaches its layout",std::abs(composer->width()-650)<.5&&std::abs(composer->height()-535)<.5);
            composer->setProperty("expanded",true);
            QTimer::singleShot(70,this,[composer]{composer->setProperty("expanded",false);});
            QTimer::singleShot(140,this,[this,composer]{
                composer->setProperty("expanded",true);
                whenStill(composer,[this,composer]{
                    check("interrupted maximize reaches full bounds",std::abs(composer->width()-(window->width()-40))<.5&&!composer->property("moving").toBool());
                    composer->setProperty("expanded",false);
                    whenStill(composer,[this,composer]{
                        check("restore retains bound layout",std::abs(composer->width()-650)<.5&&!composer->property("moving").toBool());
                        QMetaObject::invokeMethod(composer,"close");
                        check("close starts a downward movement",composer->property("moving").toBool());
                        QTimer::singleShot(70,this,[this,composer]{
                            const auto before=composer->position();const auto size=composer->size();
                            QMetaObject::invokeMethod(composer,"open",Q_ARG(QVariant,"new"));
                            check("reopening during close preserves current bounds",QLineF(before,composer->position()).length()<.1&&std::abs(composer->width()-size.width())<.1&&std::abs(composer->height()-size.height())<.1);
                            whenStill(composer,[this,composer]{
                                QMetaObject::invokeMethod(composer,"close");
                                whenStill(composer,[this,composer]{
                                    check("closed composer releases its visible surface",!composer->isVisible()&&!composer->property("moving").toBool());
                                    const auto atRest=totalFrames;
                                    QTimer::singleShot(300,this,[this,atRest]{window->setProperty("idleFrameCount",totalFrames-atRest);check("settled scene stops requesting frames",totalFrames-atRest<=2);finish();});
                                });
                            });
                        });
                    });
                });
            });
        });
    });
}
void MotionSmoke::whenStill(QQuickItem*item,std::function<void()> next,int attempts){
    if(finished)return;
    if(!item->property("moving").toBool()){next();return;}
    if(attempts<=0){check("composer springs settle within two seconds",false);finish();return;}
    QTimer::singleShot(25,this,[this,item,next,attempts]{whenStill(item,next,attempts-1);});
}
void MotionSmoke::finish() {
    if(finished)return;finished=true;
    QJsonArray activeSprings;QSet<Motion*>springs;for(auto spring:window->findChildren<Motion*>())springs.insert(spring);if(auto engine=qmlEngine(window))for(auto spring:engine->findChildren<Motion*>())springs.insert(spring);
    for(auto spring:springs)if(spring->running())activeSprings.append(QJsonObject{{"parent",spring->parent()?spring->parent()->metaObject()->className():"none"},{"value",spring->value()},{"target",spring->target()}});
    QJsonArray intervals;
    for(qsizetype i=1;i<frames.size();++i){auto interval=frames[i]-frames[i-1];if(interval>0&&interval<100)intervals.append(interval);}
    QJsonObject report{{"passed",!failed},{"syntheticData",true},{"checks",checks},{"frameIntervalsMs",intervals},{"windowExposed",window->isExposed()},{"windowWidth",window->width()},{"windowHeight",window->height()},{"elapsedMs",elapsed.isValid()?elapsed.elapsed():0},{"note","Observed frame delivery during this native smoke run; not a cross-platform frame-rate or resource guarantee."}};
    report.insert("activeSprings",activeSprings);
    report.insert("idleFrameCount",window->property("idleFrameCount").toInt());
    if(auto path=qEnvironmentVariable("LUMA_MOTION_REPORT");!path.isEmpty()){
        QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}
    }
    qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);
    bridge->quit(failed?2:0);
}
