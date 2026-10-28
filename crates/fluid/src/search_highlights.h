#pragma once
#include <QColor>
#include <QRegularExpression>
#include <QString>

// Presentation only: Gmail still owns matching and pagination. No index/cache.
class SearchHighlights {
public:
    QString render(const QString&plain,const QString&query,const QColor&colour)const;
private:
    mutable bool initialized=false;
    mutable QString previous;
    mutable QRegularExpression expression;
};
