#pragma once
#include <QQuickImageProvider>
#include <QSvgRenderer>
#include <QPainter>
#include <QRegularExpression>
#include <algorithm>
// Only the software scene-graph fallback uses CPU-tinted icons. Normal GPU
// rendering keeps its cheap shader tint and shares the vector resources.
class SoftwareIcons final:public QQuickImageProvider {
public:
    SoftwareIcons():QQuickImageProvider(QQuickImageProvider::Image){}
    QImage requestImage(const QString&id,QSize*size,const QSize&requested)override{
        const auto parts=id.split('/');if(parts.size()!=2||!QRegularExpression("^[a-z0-9-]{1,40}$").match(parts[1]).hasMatch())return {};
        const QColor colour("#"+parts[0]);if(!colour.isValid())return {};
        const int width=std::clamp(requested.width()>0?requested.width():24,1,256),height=std::clamp(requested.height()>0?requested.height():24,1,256);
        QSvgRenderer svg(":/icons/luma-"+parts[1]+"-symbolic.svg");if(!svg.isValid())return {};
        QImage image(width,height,QImage::Format_ARGB32_Premultiplied);image.fill(Qt::transparent);QPainter painter(&image);svg.render(&painter);painter.setCompositionMode(QPainter::CompositionMode_SourceIn);painter.fillRect(image.rect(),colour);painter.end();if(size)*size=image.size();return image;
    }
};
