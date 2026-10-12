extern crate thallium_backend;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
	cxx_qt::init_crate!(thallium_backend);
	let mut application = QGuiApplication::new();
	// The Wayland app id, which docks and the compositor match to
	// io.thallium.Files.desktop for the name and icon.
	QGuiApplication::set_desktop_file_name(&QString::from("io.thallium.Files"));
	let mut engine = QQmlApplicationEngine::new();
	if let Some(engine) = engine.as_mut() {
		engine.load(&QUrl::from("qrc:/qt/qml/org/thallium/FileManager/qml/main.qml"));
	}
	if let Some(application) = application.as_mut() {
		application.exec();
	}
}
