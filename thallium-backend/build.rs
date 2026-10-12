fn main() {
	if std::env::var_os("CARGO_FEATURE_QT").is_none() {
		return;
	}

	let mut resources: Vec<String> = Vec::new();
	for folder in ["qml/fonts", "qml/icons"] {
		println!("cargo::rerun-if-changed={folder}");
		let mut files: Vec<String> = std::fs::read_dir(folder)
			.expect("asset folder")
			.filter_map(Result::ok)
			.map(|entry| entry.path().to_string_lossy().into_owned())
			.filter(|path| path.ends_with(".ttf") || path.ends_with(".svg"))
			.collect();
		files.sort();
		resources.extend(files);
	}

	cxx_qt_build::CxxQtBuilder::new_qml_module(
		cxx_qt_build::QmlModule::new("org.thallium.FileManager")
			.qml_file("qml/main.qml")
			.qml_file(cxx_qt_build::QmlFile::from("qml/Theme.qml").singleton(true))
			.qml_files(["qml/CrossRule.qml", "qml/Icon.qml"]),
	)
	.qrc_resources(resources)
	.files(["src/qt_bridge.rs"])
	.include_dir("cpp")
	.qt_module("DBus")
	.cpp_file("cpp/clipboard.cpp")
	.cpp_file("cpp/filemanager1.h")
	.cpp_file("cpp/filemanager1.cpp")
	.build();
}
