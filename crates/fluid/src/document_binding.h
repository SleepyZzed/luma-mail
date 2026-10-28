#pragma once
#include <QAccessible>
#include <QQuickTextDocument>
#include <QTextDocument>

// Qt Quick's accessibility interface snapshots its document pointer. An
// interface can already exist while a QML editor is being constructed, before
// our private document is attached. Invalidate it before Qt frees that original
// document, so native text ranges cannot retain a dangling pointer.
inline void bindPrivateDocument(QQuickTextDocument*handle,QTextDocument*document){
    auto editor=handle->parent();
#if QT_CONFIG(accessibility)
    if(auto previous=QAccessible::queryAccessibleInterface(editor))
        QAccessible::deleteAccessibleInterface(QAccessible::uniqueId(previous));
#endif
    // The document must also survive the editor's wrapper/control teardown.
    document->setParent(editor?editor:handle);
    handle->setTextDocument(document);
#if QT_CONFIG(accessibility)
    if(editor){QAccessibleEvent changed(editor,QAccessible::ObjectReorder);QAccessible::updateAccessibility(&changed);}
#endif
}
