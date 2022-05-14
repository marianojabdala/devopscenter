PROJECT_NAME=devopscenter
SOURCE_CODE=devopscenter

deps:
	pip install poetry --upgrade
	poetry install

lint:
	PYTHONPATH=. pylint -v devopscenter/ | pylint-json2html -f jsonextended -o pylint.html

lint_with_text:
	PYTHONPATH=. pylint -v -f text devopscenter/

analyze:
	bandit -r -v devopscenter
format:
	yapf -i --recursive $(SOURCE_CODE)

diff:
	yapf --recursive --diff $(SOURCE_CODE)
