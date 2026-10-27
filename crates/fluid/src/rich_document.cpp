#include "rich_document.h"
#include <QAbstractTextDocumentLayout>
#include <QRegularExpression>
#include <QTextBlock>
#include <QTextFragment>
#include <QTextList>
#include <QTextTable>
#include <QUrl>
#include <algorithm>
#include <memory>

namespace {constexpr qint64 HistoryBudget=1024*1024;constexpr int MaxHistory=64;constexpr int MaxCharacters=250000;}
RichDocument::RichDocument(QObject *parent):QObject(parent),doc(new PrivateDocument(this)) {
    QFont font=QFontDatabase::systemFont(QFontDatabase::GeneralFont);font.setPointSizeF(11);doc->setDefaultFont(font);
    doc->setUndoRedoEnabled(false);doc->setDocumentMargin(0);doc->documentLayout();
    connect(doc,&QTextDocument::contentsChange,this,&RichDocument::changed);
    historyTimer.setSingleShot(true);historyTimer.setInterval(250);
    connect(&historyTimer,&QTimer::timeout,this,&RichDocument::checkpoint);
    checkpoint();
}
void RichDocument::setDocument(QQuickTextDocument *value){
    if(quick==value)return;quick=value;
    if(quick){doc->setParent(quick);quick->setTextDocument(doc);}
    emit documentChanged();
}
QTextCursor RichDocument::cursor()const {
    QTextCursor c(doc);const int limit=std::max(0,doc->characterCount()-1);
    c.setPosition(std::clamp(start,0,limit));c.setPosition(std::clamp(end,0,limit),QTextCursor::KeepAnchor);
    if(start==end)c.setPosition(std::clamp(position,0,limit));return c;
}
void RichDocument::selection(int from,int to,int caret){
    if(from!=to||(caret!=position&&caret!=lastInserted))pending=QTextCharFormat();
    start=from;end=to;position=caret;emit formatChanged();
}
QTextCharFormat RichDocument::currentFormat()const {auto f=cursor().charFormat();f.merge(pending);return f;}
bool RichDocument::bold()const{return currentFormat().fontWeight()>=QFont::Bold;}
bool RichDocument::italic()const{return currentFormat().fontItalic();}
bool RichDocument::underline()const{return currentFormat().fontUnderline();}
QString RichDocument::fontFamily()const{const auto names=currentFormat().fontFamilies().toStringList();return names.isEmpty()?doc->defaultFont().family():names.first();}
qreal RichDocument::fontPointSize()const{const auto size=currentFormat().fontPointSize();return (size>0?size:doc->defaultFont().pointSizeF())/scale;}
void RichDocument::apply(const QTextCharFormat &format){
    if(readOnly)return;checkpoint();rich=true;
    auto c=cursor();if(c.hasSelection()){pending=QTextCharFormat();c.mergeCharFormat(format);checkpoint();}
    else{pending.merge(format);}
    emit formatChanged();emit placeCursor(start,end);
}
void RichDocument::toggle(const QString &style){
    QTextCharFormat f;
    if(style=="bold")f.setFontWeight(bold()?QFont::Normal:QFont::Bold);
    else if(style=="italic")f.setFontItalic(!italic());
    else if(style=="underline")f.setFontUnderline(!underline());
    else if(style=="strike")f.setFontStrikeOut(!currentFormat().fontStrikeOut());
    else if(style=="super")f.setVerticalAlignment(currentFormat().verticalAlignment()==QTextCharFormat::AlignSuperScript?QTextCharFormat::AlignNormal:QTextCharFormat::AlignSuperScript);
    else if(style=="sub")f.setVerticalAlignment(currentFormat().verticalAlignment()==QTextCharFormat::AlignSubScript?QTextCharFormat::AlignNormal:QTextCharFormat::AlignSubScript);
    else return;apply(f);
}
void RichDocument::family(const QString &name){if(name.isEmpty()||name.size()>128||!fonts().contains(name))return;QTextCharFormat f;f.setFontFamilies({name});apply(f);}
void RichDocument::pointSize(qreal size){if(!qIsFinite(size)||size<8||size>72)return;QTextCharFormat f;f.setFontPointSize(size*scale);apply(f);}
void RichDocument::foreground(const QColor &colour){if(!colour.isValid())return;QTextCharFormat f;f.setForeground(colour);apply(f);}
void RichDocument::highlight(const QColor &colour){if(!colour.isValid())return;QTextCharFormat f;f.setBackground(colour);apply(f);}
void RichDocument::align(int alignment){
    if(readOnly)return;
    if(alignment!=Qt::AlignLeft&&alignment!=Qt::AlignRight&&alignment!=Qt::AlignCenter&&alignment!=Qt::AlignJustify)return;
    checkpoint();rich=true;auto c=cursor();auto f=c.blockFormat();f.setAlignment(Qt::Alignment(alignment));c.mergeBlockFormat(f);checkpoint();emit placeCursor(start,end);
}
void RichDocument::list(bool numbered){
    if(readOnly)return;checkpoint();rich=true;auto c=cursor();c.beginEditBlock();
    const auto style=numbered?QTextListFormat::ListDecimal:QTextListFormat::ListDisc;
    if(c.currentList()&&c.currentList()->format().style()==style){
        auto b=doc->findBlock(c.selectionStart());const int finish=c.selectionEnd();
        while(b.isValid()&&b.position()<=finish){if(auto list=b.textList())list->remove(b);QTextCursor p(b);auto f=p.blockFormat();f.setIndent(0);f.setObjectIndex(-1);p.setBlockFormat(f);b=b.next();}
    }else{QTextListFormat f;f.setStyle(style);f.setIndent(1);c.createList(f);}
    c.endEditBlock();checkpoint();emit placeCursor(start,end);
}
bool RichDocument::inList()const{return cursor().currentList()!=nullptr;}
void RichDocument::indent(int direction){
    if(readOnly)return;checkpoint();rich=true;auto c=cursor();
    if(auto list=c.currentList()){
        auto format=list->format();const int level=format.indent()+direction;
        if(level<=0){list->remove(c.block());auto f=c.blockFormat();f.setIndent(0);f.setObjectIndex(-1);c.setBlockFormat(f);}
        else{format.setIndent(std::min(level,8));c.createList(format);}
    }else{auto f=c.blockFormat();f.setLeftMargin(std::clamp(f.leftMargin()+direction*24*scale,qreal(0),qreal(192)*scale));c.mergeBlockFormat(f);}
    checkpoint();emit placeCursor(start,end);
}
bool RichDocument::paragraphBreak(){
    if(readOnly)return false;auto c=cursor();auto list=c.currentList();if(!list)return false;
    checkpoint();c.beginEditBlock();
    if(c.block().text().trimmed().isEmpty()){
        list->remove(c.block());auto f=c.blockFormat();f.setObjectIndex(-1);f.setIndent(0);c.setBlockFormat(f);
    }else c.insertBlock(c.blockFormat(),c.charFormat());
    c.endEditBlock();position=start=end=c.position();checkpoint();emit placeCursor(position,position);return true;
}
bool RichDocument::link(const QString &raw,const QString &label){
    if(readOnly)return false;QUrl url(raw,QUrl::StrictMode);
    if(!url.isValid()||raw.size()>8192||!QStringList{"http","https","mailto"}.contains(url.scheme())||!url.userInfo().isEmpty()){
        emit error("Use an HTTP, HTTPS or mailto link without embedded credentials.");return false;
    }
    checkpoint();auto c=cursor();QString text=label.isEmpty()?c.selectedText():label;if(text.isEmpty())text=raw;
    if(text.size()>8192)return false;
    QTextCharFormat f=c.charFormat();f.setAnchor(true);f.setAnchorHref(url.toString());f.setForeground(QColor("#1a73e8"));f.setFontUnderline(true);
    c.insertText(text,f);position=start=end=c.position();pending=QTextCharFormat();rich=true;checkpoint();emit placeCursor(position,position);return true;
}
void RichDocument::removeLink(const QString &url){
    if(readOnly)return;checkpoint();QList<QPair<int,int>>ranges;
    for(auto b=doc->begin();b.isValid();b=b.next())for(auto i=b.begin();!i.atEnd();++i){auto f=i.fragment();if(f.isValid()&&f.charFormat().anchorHref()==url)ranges.append({f.position(),f.length()});}
    for(auto i=ranges.crbegin();i!=ranges.crend();++i){QTextCursor c(doc);c.setPosition(i->first);c.setPosition(i->first+i->second,QTextCursor::KeepAnchor);c.removeSelectedText();}
    checkpoint();
}
void RichDocument::clearFormat(){
    if(readOnly)return;checkpoint();auto c=cursor();QTextCharFormat f;
    if(c.hasSelection())c.setCharFormat(f);else{
        f.setFont(doc->defaultFont());f.setFontWeight(QFont::Normal);f.setFontItalic(false);f.setFontUnderline(false);f.setFontStrikeOut(false);f.setVerticalAlignment(QTextCharFormat::AlignNormal);f.setAnchor(false);f.setAnchorHref({});f.setForeground(QBrush(Qt::NoBrush));f.setBackground(QBrush(Qt::NoBrush));pending=f;
    }
    auto block=c.blockFormat();block.setAlignment(Qt::AlignLeft);block.setLeftMargin(0);c.mergeBlockFormat(block);
    checkpoint();emit formatChanged();emit placeCursor(start,end);
}
void RichDocument::scaleFonts(QTextDocument *document,qreal ratio)const {
    auto base=document->defaultFont();if(base.pointSizeF()>0)base.setPointSizeF(base.pointSizeF()*ratio);document->setDefaultFont(base);
    struct Change{int position,length;QTextCharFormat format;};QList<Change>changes;
    for(auto b=document->begin();b.isValid();b=b.next()){
        for(auto i=b.begin();!i.atEnd();++i){const auto fragment=i.fragment();if(!fragment.isValid())continue;
            auto f=fragment.charFormat();if(f.fontPointSize()>0)f.setFontPointSize(f.fontPointSize()*ratio);
            else if(f.font().pixelSize()>0){auto font=f.font();font.setPixelSize(qRound(font.pixelSize()*ratio));f.setFont(font,QTextCharFormat::FontPropertiesSpecifiedOnly);}
            changes.append({fragment.position(),fragment.length(),f});
        }
        QTextCursor c(b);auto f=c.blockFormat();f.setLeftMargin(f.leftMargin()*ratio);f.setRightMargin(f.rightMargin()*ratio);f.setTopMargin(f.topMargin()*ratio);f.setBottomMargin(f.bottomMargin()*ratio);c.setBlockFormat(f);
    }
    for(const auto &change:changes){QTextCursor c(document);c.setPosition(change.position);c.setPosition(change.position+change.length,QTextCursor::KeepAnchor);c.setCharFormat(change.format);}
}
void RichDocument::setUiScale(qreal value){
    value=std::clamp(value,qreal(.75),qreal(2));if(qFuzzyCompare(value,scale))return;
    mute=true;scaleFonts(doc,value/scale);scale=value;mute=false;emit scaleChanged();emit formatChanged();
}
void RichDocument::load(const QVariantMap &value){
    historyTimer.stop();mute=true;pending=QTextCharFormat();start=end=position=0;
    const QString text=value.value("text").toString(),kind=value.value("kind").toString();
    if(text.size()>1024*1024){mute=false;emit error("The document exceeds the editor memory limit. Keep the original in Gmail.");return;}
    rich=kind!="plain";doc->clear();auto base=doc->defaultFont();base.setPointSizeF(11);doc->setDefaultFont(base);
    if(kind=="legacy")legacy(value);else if(rich)doc->setHtml(text);else doc->setPlainText(text);
    if(scale!=1)scaleFonts(doc,scale);
    doc->setUndoRedoEnabled(false);doc->setModified(false);mute=false;
    history.clear();historyAt=-1;checkpoint();emit placeCursor(0,0);emit formatChanged();
}
void RichDocument::legacy(const QVariantMap &value){
    const auto text=value.value("text").toString();doc->setPlainText(text);
    QList<int>offsets;for(int i=0;i<text.size();){offsets.append(i);i+=text[i].isHighSurrogate()&&i+1<text.size()&&text[i+1].isLowSurrogate()?2:1;}offsets.append(text.size());
    auto spans=value.value("spans").toList();std::stable_sort(spans.begin(),spans.end(),[](const QVariant &a,const QVariant &b){auto x=a.toMap(),y=b.toMap();return x["end"].toInt()-x["start"].toInt()>y["end"].toInt()-y["start"].toInt();});
    for(const auto &item:spans){auto span=item.toMap();int a=span["start"].toInt(),b=span["end"].toInt();if(a<0||b<a||b>=offsets.size())continue;
        QTextCursor c(doc);c.setPosition(offsets[a]);c.setPosition(offsets[b],QTextCursor::KeepAnchor);const auto tag=span["tag"].toString();QTextCharFormat f;
        if(tag=="strong")f.setFontWeight(QFont::Bold);else if(tag=="em")f.setFontItalic(true);else if(tag=="u")f.setFontUnderline(true);else if(tag=="s")f.setFontStrikeOut(true);
        else if(tag.startsWith("font:family:"))f.setFontFamilies({tag.mid(12)});else if(tag.startsWith("font:size:"))f.setFontPointSize(tag.mid(10).toDouble());
        else if(tag.startsWith("link:")){f.setAnchor(true);f.setAnchorHref(tag.mid(5));f.setForeground(QColor("#1a73e8"));f.setFontUnderline(true);}
        else if(tag.startsWith("style:color:"))f.setForeground(QColor(tag.mid(12)));else if(tag.startsWith("style:highlight:"))f.setBackground(QColor(tag.mid(16)));
        else if(tag=="style:script:super")f.setVerticalAlignment(QTextCharFormat::AlignSuperScript);else if(tag=="style:script:sub")f.setVerticalAlignment(QTextCharFormat::AlignSubScript);
        else if(tag=="style:code:1")f.setFontFamilies({QFontDatabase::systemFont(QFontDatabase::FixedFont).family()});
        else if(tag.startsWith("paragraph:align:")){auto block=c.blockFormat();auto v=tag.mid(16);block.setAlignment(v=="center"?Qt::AlignCenter:v=="right"?Qt::AlignRight:v=="justify"?Qt::AlignJustify:Qt::AlignLeft);c.mergeBlockFormat(block);}
        else if(tag.startsWith("paragraph:indent:")){auto block=c.blockFormat();block.setLeftMargin(tag.mid(17).toInt()*24);c.mergeBlockFormat(block);}
        c.mergeCharFormat(f);
    }
    const QRegularExpression prefix(QString::fromUtf8("^( *)(• |[0-9]{1,6}\\. )"));
    for(auto b=doc->begin();b.isValid();b=b.next()){
        auto match=prefix.match(b.text());if(!match.hasMatch())continue;QTextCursor c(b);c.setPosition(b.position()+match.capturedLength(),QTextCursor::KeepAnchor);c.removeSelectedText();
        QTextListFormat f;f.setIndent(std::min(8,int(match.captured(1).size()/2+1)));f.setStyle(match.captured(2).startsWith(QChar(0x2022))?QTextListFormat::ListDisc:QTextListFormat::ListDecimal);
        if(auto previous=b.previous();previous.isValid()&&previous.textList()&&previous.textList()->format().style()==f.style()&&previous.textList()->format().indent()==f.indent())previous.textList()->add(b);else c.createList(f);
    }
}
QString RichDocument::html()const {
    std::unique_ptr<QTextDocument>copy(doc->clone());copy->setUndoRedoEnabled(false);if(scale!=1)scaleFonts(copy.get(),1/scale);
    QString html=copy->toHtml();const auto begin=html.indexOf("<body");const auto open=html.indexOf('>',begin);const auto close=html.lastIndexOf("</body>");
    if(begin>=0&&open>=0&&close>open)html=html.mid(open+1,close-open-1);
    const QRegularExpression indent("-qt-block-indent:([0-9]+)");
    auto matches=indent.globalMatch(html);QList<QRegularExpressionMatch> replacements;while(matches.hasNext())replacements.append(matches.next());
    for(auto i=replacements.crbegin();i!=replacements.crend();++i)html.replace(i->capturedStart(),i->capturedLength(),"margin-left:"+QString::number(i->captured(1).toInt()*24)+"px");
    return html;
}
QVariantMap RichDocument::payload()const{return {{"text",rich?html():doc->toPlainText()},{"html",rich}};}
void RichDocument::changed(int at,int,int added){
    if(mute||!doc)return;
    if(doc->characterCount()>MaxCharacters){emit error("The editor supports up to 250,000 characters per message.");QTimer::singleShot(0,this,[this]{restore(historyAt);});return;}
    if(added>0&&!pending.isEmpty()){
        mute=true;QTextCursor c(doc);c.setPosition(at);c.setPosition(std::min(at+added,doc->characterCount()-1),QTextCursor::KeepAnchor);c.mergeCharFormat(pending);mute=false;
    }
    lastInserted=at+added;historyTimer.start();emit edited();emit formatChanged();
}
qint64 RichDocument::undoBytes()const{qint64 bytes=0;for(const auto &s:history)bytes+=s.html.size()*2;return bytes;}
void RichDocument::checkpoint(){
    if(mute||!doc)return;historyTimer.stop();Snapshot now{html(),position,rich};
    if(historyAt>=0&&history[historyAt].html==now.html)return;
    while(history.size()>historyAt+1)history.removeLast();
    if(now.html.size()*2>HistoryBudget){history.clear();historyAt=-1;emit historyChanged();return;}
    history.append(now);historyAt=history.size()-1;
    while(history.size()>MaxHistory||undoBytes()>HistoryBudget){history.removeFirst();--historyAt;}
    emit historyChanged();
}
void RichDocument::restore(int at){
    if(at<0||at>=history.size())return;historyTimer.stop();mute=true;const auto s=history[at];auto base=doc->defaultFont();base.setPointSizeF(11);doc->setDefaultFont(base);doc->setHtml(s.html);if(scale!=1)scaleFonts(doc,scale);rich=s.rich;
    position=start=end=std::min(s.cursor,doc->characterCount()-1);pending=QTextCharFormat();historyAt=at;mute=false;emit placeCursor(position,position);emit edited();emit historyChanged();emit formatChanged();
}
void RichDocument::undo(){if(readOnly)return;checkpoint();if(canUndo())restore(historyAt-1);}
void RichDocument::redo(){if(readOnly)return;if(canRedo())restore(historyAt+1);}
bool RichDocument::find(const QString &term,bool backwards,bool matchCase,bool whole){
    if(term.isEmpty()||term.size()>8192)return false;QTextDocument::FindFlags flags;
    if(backwards)flags|=QTextDocument::FindBackward;if(matchCase)flags|=QTextDocument::FindCaseSensitively;if(whole)flags|=QTextDocument::FindWholeWords;
    auto found=doc->find(term,backwards?start:end,flags);if(found.isNull())found=doc->find(term,backwards?doc->characterCount()-1:0,flags);
    if(found.isNull())return false;start=found.selectionStart();end=found.selectionEnd();position=found.position();emit placeCursor(start,end);return true;
}
int RichDocument::replace(const QString &term,const QString &replacement,bool all,bool matchCase,bool whole){
    if(readOnly||term.isEmpty()||term.size()>8192||replacement.size()>8192)return 0;checkpoint();QTextDocument::FindFlags flags;
    if(matchCase)flags|=QTextDocument::FindCaseSensitively;if(whole)flags|=QTextDocument::FindWholeWords;
    auto c=doc->find(term,all?0:start,flags);int count=0;QTextCursor group(doc);group.beginEditBlock();
    while(!c.isNull()&&count<10000){if(doc->characterCount()+replacement.size()-term.size()>MaxCharacters)break;c.insertText(replacement);++count;if(!all)break;c=doc->find(term,c.position(),flags);}
    group.endEditBlock();checkpoint();return count;
}
