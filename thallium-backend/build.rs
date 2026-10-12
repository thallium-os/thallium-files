fn main() {
	if std::env::var_os("CARGO_FEATURE_QT").is_none() {
		return;
	}

	cxx_qt_build::CxxQtBuilder::new_qml_module(
		cxx_qt_build::QmlModule::new("org.thallium.FileManager").qml_file("qml/main.qml"),
	)
	.files(["src/qt_bridge.rs"])
	.include_dir("cpp")
	.cpp_file("cpp/clipboard.cpp")
	.build();
}
