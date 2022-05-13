PROJECT_NAME=devopscenter
SOURCE_CODE=devopscenter

deps:
	pip install poetry --upgrade
	poetry install

lint:
	pylint -v --recursive y -j 4 devopscenter/ | pylint-json2html -f jsonextended -o pylint.html

format:
	yapf -i --recursive $(SOURCE_CODE)

diff:
	yapf --recursive --diff $(SOURCE_CODE)
