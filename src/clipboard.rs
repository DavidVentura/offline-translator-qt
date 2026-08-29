use cpp::cpp;
use qmetaobject::QString;

cpp! {{
    #include <QtGui/QClipboard>
    #include <QtGui/QGuiApplication>
}}

pub fn set_text(text: QString) {
    cpp!(unsafe [text as "QString"] {
        QGuiApplication::clipboard()->setText(text);
    });
}
