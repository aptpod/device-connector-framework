pub mod pytypes;

mod exec_element;
mod simple_executor;

dc_core::define_plugin!(
    "python";
    exec_element::PythonSrcElement,
    exec_element::PythonFilterElement,
    exec_element::PythonSinkElement,
);
