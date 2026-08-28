extern crate thallium_backend;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
	cxx_qt::init_crate!(thallium_backend);
	let mut application = QGuiApplication::new();
	let mut engine = QQmlApplicationEngine::new();
	if let Some(engine) = engine.as_mut() {
		engine.load(&QUrl::from("qrc:/qt/qml/org/thallium/FileManager/qml/main.qml"));
	}
	if let Some(application) = application.as_mut() {
		application.exec();
	}
}
